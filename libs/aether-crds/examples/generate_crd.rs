use aether_crds::v1alpha::identity_instance::IdentityInstance;
use aether_crds::v1alpha::identity_instance_upgrade::IdentityInstanceUpgrade;
use kube::CustomResourceExt;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());

    let crds = match mode.as_str() {
        "identity-instance" => vec![IdentityInstance::crd()],
        "identity-instance-upgrade" => vec![IdentityInstanceUpgrade::crd()],
        "all" => vec![IdentityInstance::crd(), IdentityInstanceUpgrade::crd()],
        other => {
            eprintln!(
                "Unknown mode `{}`. Use one of: identity-instance, identity-instance-upgrade, all",
                other
            );
            std::process::exit(2);
        }
    };

    for (index, crd) in crds.iter().enumerate() {
        let yaml = serde_yaml::to_string(crd).expect("Failed to serialize CRD to YAML");
        print!("{}", yaml);
        if index + 1 < crds.len() {
            println!("---");
        }
    }
}
