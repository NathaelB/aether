use aether_crds::v1alpha::identity_instance::IdentityInstance;
use kube::CustomResourceExt;

fn main() {
    let crd = IdentityInstance::crd();

    let yaml = serde_yaml::to_string(&crd).expect("Failed to serialize CRD to YAML");

    println!("{}", yaml);
}
