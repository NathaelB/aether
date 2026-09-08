//! Attribute macros for the persistence seam.
//!
//! Two problems are solved here, and they are the same problem seen from each
//! side of the boundary.
//!
//! A repository used to carry a `PgExecutor::{Pool, Tx}` enum, so every method
//! matched on its executor and wrote the same SQL twice — 38 duplicated
//! statements across six repositories. [`macro@repository`] registers a
//! repository that only ever holds a transaction, so each query is written
//! once.
//!
//! A use case used to open a transaction by hand, wrap it in an
//! `Arc<Mutex<Option<_>>>`, build each repository from it, and drive the whole
//! thing through a boxed closure. [`macro@transactional`] does that, leaving
//! the body to say what the use case actually does.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemFn, ItemStruct, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Registers a repository as the implementation a backend provides for a
/// domain.
///
/// Expands to a `RepoFor<Domain>` impl on the backend marker, which is how
/// [`macro@transactional`] resolves `deployment` to
/// `PostgresDeploymentRepository` by type rather than by rebuilding an
/// identifier from a string and hoping the path exists. A domain nobody
/// registered is a compile error naming that domain.
///
/// # Requirements
///
/// - The struct is `pub`, and has `pub fn new(tx: &SharedTx<'tx>) -> Self`.
///   The generated impl names it as a public associated type, so a private
///   struct fails with `E0446` rather than anything mentioning this macro.
/// - Markers exist under `aether_persistence::registry::{domain, backend}`.
///
/// # Example
///
/// ```ignore
/// #[repository(domain = Deployment, backend = Postgres)]
/// pub struct PostgresDeploymentRepository<'tx> { /* ... */ }
/// ```
#[proc_macro_attribute]
pub fn repository(args: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(args as RepositoryAttrs);
    let item = parse_macro_input!(input as ItemStruct);

    let struct_name = &item.ident;
    let domain = &attrs.domain;
    let backend = &attrs.backend;

    quote! {
        #item

        impl ::aether_postgres::registry::RepoFor<
            ::aether_postgres::registry::domain::#domain
        > for ::aether_postgres::registry::backend::#backend {
            type Repo<'tx> = #struct_name<'tx>;

            fn build<'tx>(
                tx: &::aether_persistence::SharedTx<'tx>,
            ) -> Self::Repo<'tx> {
                #struct_name::new(tx)
            }
        }
    }
    .into()
}

/// Wraps an async method body in one transaction, binding a repository for
/// each domain listed.
///
/// Each snake_case name is pascal-cased, looked up under `registry::domain`,
/// resolved through the active `Backend`, and bound as `{name}_repository`.
/// The body commits on `Ok` and rolls back on `Err` without saying so.
///
/// # Conventions
///
/// - `self` exposes `pool(&self) -> &PgPool`.
/// - Every listed domain has a registration (see [`macro@repository`]).
/// - The transaction handle is bound as `tx`, so a body needing a second
///   instance of the same repository — two views onto one table, say — can
///   build it with `Repository::new(&tx)` instead of listing the domain twice.
///   A body variable named `tx` would be shadowed by it.
///
/// # Example
///
/// ```ignore
/// #[transactional(deployment, user, dataplane, action)]
/// pub async fn create_deployment(
///     &self,
///     command: CreateDeploymentCommand,
/// ) -> Result<Deployment, CoreError> {
///     let service = DeploymentServiceImpl::new(
///         deployment_repository, user_repository, dataplane_repository,
///     );
///     let deployment = service.create_deployment(command).await?;
///     ActionServiceImpl::new(action_repository).record_action(/* ... */).await?;
///     Ok(deployment)
/// }
/// ```
#[proc_macro_attribute]
pub fn transactional(args: TokenStream, input: TokenStream) -> TokenStream {
    let domains = parse_macro_input!(args as DomainList);
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = parse_macro_input!(input as ItemFn);

    let bindings = domains.names.iter().map(|name| {
        let binding = Ident::new(&format!("{name}_repository"), name.span());
        let domain_ty = Ident::new(&pascal_case(&name.to_string()), name.span());
        quote! {
            let #binding = <
                ::aether_postgres::registry::Backend
                as ::aether_postgres::registry::RepoFor<
                    ::aether_postgres::registry::domain::#domain_ty
                >
            >::build(&tx);
        }
    });

    let body_stmts = &block.stmts;

    quote! {
        #(#attrs)*
        #vis #sig {
            ::aether_persistence::with_tx(
                self.pool(),
                ::aether_postgres::map_sqlx_error,
                async |tx| {
                    #(#bindings)*
                    #(#body_stmts)*
                },
            )
            .await
        }
    }
    .into()
}

struct RepositoryAttrs {
    domain: Ident,
    backend: Ident,
}

impl Parse for RepositoryAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut domain = None;
        let mut backend = None;

        for KeyValue { key, value } in Punctuated::<KeyValue, Token![,]>::parse_terminated(input)? {
            match key.to_string().as_str() {
                "domain" => domain = Some(value),
                "backend" => backend = Some(value),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key `{other}`, expected `domain` or `backend`"),
                    ));
                }
            }
        }

        Ok(Self {
            domain: domain
                .ok_or_else(|| syn::Error::new(input.span(), "missing required key `domain`"))?,
            backend: backend
                .ok_or_else(|| syn::Error::new(input.span(), "missing required key `backend`"))?,
        })
    }
}

struct KeyValue {
    key: Ident,
    value: Ident,
}

impl Parse for KeyValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let value = input.parse::<Ident>()?;
        Ok(Self { key, value })
    }
}

struct DomainList {
    names: Vec<Ident>,
}

impl Parse for DomainList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { names: Vec::new() });
        }
        Ok(Self {
            names: Punctuated::<Ident, Token![,]>::parse_terminated(input)?
                .into_iter()
                .collect(),
        })
    }
}

fn pascal_case(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper_next = true;
    for c in snake.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::pascal_case;

    #[test]
    fn pascal_case_maps_snake_case_domain_names() {
        assert_eq!(pascal_case("deployment"), "Deployment");
        assert_eq!(pascal_case("data_plane"), "DataPlane");
        assert_eq!(pascal_case("organisation"), "Organisation");
    }

    #[test]
    fn pascal_case_leaves_an_already_pascal_name_alone() {
        assert_eq!(pascal_case("DataPlane"), "DataPlane");
    }

    #[test]
    fn pascal_case_handles_the_degenerate_inputs() {
        assert_eq!(pascal_case(""), "");
        assert_eq!(pascal_case("_"), "");
        assert_eq!(pascal_case("a"), "A");
    }
}
