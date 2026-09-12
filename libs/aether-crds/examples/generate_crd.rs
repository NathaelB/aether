use aether_crds::v1alpha::identity_instance::IdentityInstance;
use aether_crds::v1alpha::identity_instance_backup::{
    IdentityInstanceBackup, IdentityInstanceBackupSchedule,
};
use aether_crds::v1alpha::identity_instance_upgrade::IdentityInstanceUpgrade;
use kube::CustomResourceExt;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());

    let crds = match mode.as_str() {
        "identity-instance" => vec![IdentityInstance::crd()],
        "identity-instance-upgrade" => vec![IdentityInstanceUpgrade::crd()],
        "identity-instance-backup" => vec![
            IdentityInstanceBackup::crd(),
            IdentityInstanceBackupSchedule::crd(),
        ],
        "all" => vec![
            IdentityInstance::crd(),
            IdentityInstanceUpgrade::crd(),
            IdentityInstanceBackup::crd(),
            IdentityInstanceBackupSchedule::crd(),
        ],
        other => {
            eprintln!(
                "Unknown mode `{}`. Use one of: identity-instance, identity-instance-upgrade, identity-instance-backup, all",
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
