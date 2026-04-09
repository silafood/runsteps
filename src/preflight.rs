use crate::config::Step;
use anyhow::Result;
use std::process::Command;

pub fn ensure_just_available(steps: &[Step]) -> Result<()> {
    let needs_just = steps.iter().any(|s| s.just_recipe.is_some());
    if !needs_just {
        return Ok(());
    }

    match Command::new("just").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            eprintln!("  ✓ just {}", version.trim());
            Ok(())
        }
        _ => {
            eprintln!("  ✗ `just` is not installed but required by this config.");
            eprintln!();
            eprintln!("  Install it with one of:");
            eprintln!("    cargo install just");
            eprintln!("    brew install just");
            eprintln!("    curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin");
            eprintln!();

            let install = inquire::Confirm::new("Attempt `cargo install just` now?")
                .with_default(false)
                .prompt()?;

            if install {
                let status = Command::new("cargo")
                    .args(["install", "just"])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Failed to install just via cargo");
                }
                eprintln!("  ✓ just installed successfully");
                Ok(())
            } else {
                anyhow::bail!("Cannot proceed without `just`. Install it and try again.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Step;

    fn make_shell_step(name: &str) -> Step {
        Step {
            name: name.to_string(),
            description: "desc".to_string(),
            command: Some("echo test".to_string()),
            just_recipe: None,
            group: None,
            confirm: false,
            depends_on: vec![],
        }
    }

    #[test]
    fn test_no_just_steps_skips_check() {
        let steps = vec![make_shell_step("a"), make_shell_step("b")];
        assert!(ensure_just_available(&steps).is_ok());
    }
}
