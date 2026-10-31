//! Smoke test: the `auctioneer-cli` binary parses its CLI without panicking
//! and `--help` lists the built-in commands.

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn help_lists_builtin_commands() {
        let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .arg("--help")
            .output()
            .expect("run auctioneer-cli --help");

        assert!(
            output.status.success(),
            "`auctioneer-cli --help` should exit 0"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        for command in ["bidders", "gvl", "reference", "openrtb"] {
            assert!(
                stdout
                    .lines()
                    .any(|line| line.split_whitespace().next() == Some(command)),
                "missing maintenance command {command}: {stdout}"
            );
        }
        for command in [
            "active-version",
            "auth",
            "build",
            "config",
            "deploy",
            "healthcheck",
            "new",
            "provision",
            "qualify",
            "rollback",
            "serve",
        ] {
            let listed = stdout
                .lines()
                .any(|line| line.split_whitespace().next() == Some(command));
            assert_eq!(
                listed,
                cfg!(feature = "application"),
                "application command {command}: {stdout}"
            );
        }
    }

    #[test]
    fn application_command_parsing_is_preserved_in_default_builds() {
        for arguments in [
            vec!["config", "--help"],
            vec!["config", "push", "--help"],
            vec!["config", "validate", "--help"],
            vec!["deploy", "--help"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .args(&arguments)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(if cfg!(feature = "application") {
                    0_i32
                } else {
                    2_i32
                }),
                "{arguments:?}: {output:?}"
            );
        }
    }

    #[test]
    fn maintenance_namespaces_expose_their_nested_commands() {
        for (arguments, commands) in [
            (vec!["gvl", "--help"], vec!["refresh"]),
            (vec!["reference", "--help"], vec!["check"]),
            (
                vec!["openrtb", "catalogue", "--help"],
                vec!["generate", "check"],
            ),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .args(&arguments)
                .output()
                .unwrap();
            assert!(output.status.success(), "{arguments:?}: {output:?}");
            let help = String::from_utf8(output.stdout).unwrap();
            for command in commands {
                assert!(help.contains(command), "{arguments:?}: {help}");
            }
        }
    }

    #[test]
    fn maintenance_refuses_unknown_subcommands_and_invalid_refresh_flags() {
        for arguments in [
            vec!["gvl", "unknown"],
            vec!["reference", "validate"],
            vec!["openrtb", "unknown"],
            vec!["gvl", "refresh", "--max-age-days", "not-a-number"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .args(arguments)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2_i32));
        }
    }
}
