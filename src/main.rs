use std::{path::PathBuf, process::Command};

use cargo_toml::{DependencyDetail, Manifest};
use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;

use clap_complete::shells::PowerShell;
use clap_complete::Generator;
use clap_complete::Shell;

use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use std::io;
use which::which;
use yansi::Paint;

#[derive(Parser, Debug, PartialEq)]
#[clap(
    name = "install-from-manifest",
    author,
    version,
    about,
    trailing_var_arg = true
)]
struct Args {
    /// Path to the manifest
    #[clap()]
    manifest_path: PathBuf,

    /// Name of a specific crate inside the manifest to install
    #[clap(short, long)]
    name: Option<String>,

    #[clap(short, long)]
    force: bool,

    #[clap(short, long)]
    verbose: bool,

    #[clap(flatten)]
    workspace: clap_cargo::Workspace,

    #[clap(flatten)]
    manifest: clap_cargo::Manifest,

    #[clap(long = "generate", arg_enum)]
    generator: Option<Shell>,
}

fn print_completions<G: Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

// use https://github.com/rust-cli/clap-verbosity-flag
// use https://github.com/crate-ci/clap-cargo
// use https://docs.rs/clap_complete/latest/clap_complete/
// use https://crates.io/crates/clap_complete_fig
// TODO use proper error handling/returning
// TODO implement verbose & silent
// TODO implement force
// TODO cicd
// TODO auto complete
fn build_args(details: &'_ DependencyDetail) -> Vec<String> {
    let mut args = Vec::new();

    if let Some(version) = &details.version {
        args.push("--version".to_string());
        args.push(version.to_owned());
    }

    if let Some(registry) = &details.registry {
        args.push("--registry".to_string());
        args.push(registry.to_owned());
    }

    if let Some(registry_index) = &details.registry_index {
        args.push("--index".to_string());
        args.push(registry_index.to_owned());
    }

    if let Some(path) = &details.path {
        args.push("--path".to_string());
        args.push(path.to_owned());
    }

    if let Some(git) = &details.git {
        args.push("--git".to_string());
        args.push(git.to_owned());
    }

    if let Some(branch) = &details.branch {
        args.push("--branch".to_string());
        args.push(branch.to_owned());
    }

    if let Some(tag) = &details.tag {
        args.push("--tag".to_string());
        args.push(tag.to_owned());
    }

    if let Some(rev) = &details.rev {
        args.push("--rev".to_string());
        args.push(rev.to_owned());
    }

    args.push("--features".to_string());
    args.push(details.features.join(","));

    if let Some(false) = details.default_features {
        args.push("--no-default-features".to_string());
    }

    args
}

#[derive(PartialEq)]
enum CommandKind {
    Binstall,
    CargoInstall,
}

impl CommandKind {
    fn subcommand(&self) -> &'static str {
        match self {
            Self::Binstall => "binstall",
            Self::CargoInstall => "install",
        }
    }
}

fn build_command(kind: CommandKind, name: String, args: Option<&[String]>) -> Command {
    let mut command = Command::new("cargo");

    command.args(&[kind.subcommand(), &name]);

    if let Some(args) = args {
        command.args(args);
    }

    if kind == CommandKind::Binstall {
        command.arg("--no-confirm");
    }

    command
}

// TODO verbose output correctly
fn handle_command_output(mut command: Command, silent: bool, verbose: bool) -> bool {
    if verbose || !silent {
        command.stdout(std::process::Stdio::inherit());
        command.stderr(std::process::Stdio::inherit());
    } else {
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
    }
    let output = command.spawn().unwrap().wait_with_output().unwrap();

    if output.status.success() && !silent {
        println!(
            "Failed during install using {:?}: {}",
            command,
            String::from_utf8(output.stdout).unwrap()
        );
    }

    output.status.success()
}

fn main() {
    print_completions(PowerShell, &mut Args::command());

    let input_args = std::env::args().collect::<Vec<_>>();

    println!("{}", input_args.join(","));

    let arguments = Args::parse();

    println!("{:?}", arguments.workspace);
    println!("{:?}", arguments.manifest);

    let path = if arguments.manifest_path.is_dir() {
        arguments.manifest_path.join("Cargo.toml")
    } else {
        arguments.manifest_path
    };

    if !path.exists() {
        eprintln!("Cargo file at {} not found", path.to_str().unwrap());
        return;
    }

    let manifest = Manifest::from_path(&path).unwrap_or_else(|_| {
        panic!(
            "Failed to parse cargo manifest at {}",
            &path.to_str().unwrap()
        )
    });

    let bar = ProgressBar::new(manifest.dependencies.len().try_into().unwrap());
    println!(
        "     {} found {} crates to install",
        Paint::green("Peeking").bold(),
        manifest.dependencies.len()
    );

    bar.set_style(
        ProgressStyle::default_bar()
            .template(&format!(
                "  {} [{{bar:25.white/white}}] {{pos:>7}}/{{len:7}} {{msg}}",
                Paint::green("Installing").bold()
            ))
            .progress_chars("=> "),
    );

    for (name, dependency) in manifest.dependencies.iter() {
        match dependency {
            cargo_toml::Dependency::Simple(version) => {
                bar.set_message(name.to_owned());
                if (which("cargo-binstall").is_err() || !handle_command_output(
                        build_command(
                            CommandKind::Binstall,
                            name.to_owned(),
                            Some(&["--version".to_string(), version.to_owned()]),
                        ),
                        true,
                        arguments.verbose,
                    )) && !handle_command_output(
                        build_command(
                            CommandKind::CargoInstall,
                            name.to_owned(),
                            Some(&["--version".to_string(), version.to_owned()]),
                        ),
                        true,
                        arguments.verbose,
                    ) {
                    panic!("Failed to install");
                }
                bar.inc(1);
            }
            cargo_toml::Dependency::Detailed(details) => {
                let name = details
                    .package
                    .to_owned()
                    .unwrap_or_else(|| name.to_owned());

                let args = build_args(details);

                if (which("cargo-binstall").is_err() || !handle_command_output(
                        build_command(CommandKind::Binstall, name.to_owned(), Some(&args)),
                        true,
                        arguments.verbose,
                    )) && !handle_command_output(
                        build_command(CommandKind::CargoInstall, name.to_owned(), Some(&args)),
                        true,
                        arguments.verbose,
                    ) {
                    panic!("Failed to install");
                }

                bar.set_message(name.to_owned());
            }
        }
    }

    bar.set_message("");
    bar.finish();

    println!("  {}", Paint::green("Finished install").bold())
}
