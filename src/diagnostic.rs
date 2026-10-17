//! Terminal diagnostics share one boundary. Keep source error chains for
//! context, but never expose Rust's debug formatting or OS error numbers.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use clap::error::{ContextKind, ErrorKind};

pub fn program() -> String {
    std::env::args_os()
        .next()
        .and_then(|name| {
            Path::new(&name)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "trail".into())
}

pub fn single_line(message: &str) -> String {
    let mut message = message.to_string();
    // Some wrapped library errors retain only an OS error's Display text.
    // Remove numeric suffixes there as well as on typed io::Error sources.
    while let Some(start) = message.rfind(" (os error ") {
        let number = start + " (os error ".len();
        let Some(end) = message[number..].find(')').map(|end| number + end) else {
            break;
        };
        if message[number..end].parse::<i32>().is_err() {
            break;
        }
        message.replace_range(start..=end, "");
    }
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
        .trim_end_matches('.')
        .to_string()
}

pub fn error(error: &anyhow::Error) {
    let message = error
        .chain()
        .map(|cause| {
            let text = cause.to_string();
            if let Some(code) = cause
                .downcast_ref::<std::io::Error>()
                .and_then(|error| error.raw_os_error())
            {
                text.trim_end_matches(&format!(" (os error {code})"))
                    .to_string()
            } else {
                object_first(&text)
            }
        })
        .collect::<Vec<_>>()
        .join(": ");
    eprintln!("{}: {}", program(), single_line(&message));
}

fn object_first(message: &str) -> String {
    for (prefix, operation) in [
        ("reading ", "read"),
        ("writing ", "write"),
        ("loading ", "load"),
        ("parsing ", "parse"),
        ("starting ", "start"),
        ("binding ", "bind"),
    ] {
        if let Some(object) = message.strip_prefix(prefix) {
            return format!("{object}: cannot {operation}");
        }
    }
    message.to_string()
}

pub fn warning(message: impl std::fmt::Display) {
    eprintln!(
        "{}: warning: {}",
        program(),
        single_line(&message.to_string())
    );
}

/// Render clap's structured errors, leaving help/version generation to clap.
pub fn arguments(error: clap::Error) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayVersion => {
            return if writeln!(
                std::io::stdout().lock(),
                "{} {}",
                program(),
                env!("CARGO_PKG_VERSION")
            )
            .is_ok()
            {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        ErrorKind::DisplayHelp => {
            return if error.print().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            return if error.print().is_ok() {
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            };
        }
        _ => {}
    }
    let context = |kind| error.get(kind).map(ToString::to_string).unwrap_or_default();
    let argument = context(ContextKind::InvalidArg);
    let value = context(ContextKind::InvalidValue);
    let message = match error.kind() {
        ErrorKind::InvalidSubcommand => format!(
            "unknown command '{}'",
            context(ContextKind::InvalidSubcommand)
        ),
        ErrorKind::UnknownArgument => format!("unknown argument '{argument}'"),
        ErrorKind::InvalidValue if value.is_empty() => format!("{argument}: missing value"),
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => {
            format!("{argument}: invalid value '{value}'")
        }
        ErrorKind::ArgumentConflict => format!("{argument}: repeated or conflicting argument"),
        ErrorKind::MissingRequiredArgument => format!("missing required argument '{argument}'"),
        ErrorKind::MissingSubcommand => "missing command".into(),
        ErrorKind::InvalidUtf8 => "arguments must contain valid UTF-8".into(),
        _ => "invalid command arguments".into(),
    };
    let program = program();
    eprintln!(
        "{program}: {}\nTry '{program} --help' for more information.",
        single_line(&message)
    );
    ExitCode::from(2)
}
