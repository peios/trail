"""Local host-tool checks and explicit developer commands.

Tests run against declared runfiles and inherit only host tool lookup and Rust
toolchain variables. They are local checks, not hermetic remote-execution targets.
Workspace commands are explicitly invoked with `bazel run`; they can format or
install files and therefore never participate in the build action cache.
"""

def _command_impl(ctx):
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    lines = [
        "#!/bin/bash",
        "set -euo pipefail",
        'runfiles="${RUNFILES_DIR:-$0.runfiles}/%s"' % ctx.workspace_name,
    ]
    for variable, target in [
        ("TRAIL_PRETTIER", ctx.file.prettier),
        ("TRAIL_BUILDIFIER", ctx.file.buildifier),
        ("TRAIL_BINARY", ctx.executable.binary),
    ]:
        if target:
            lines.append('export %s="$runfiles/%s"' % (variable, target.short_path))
    lines.append('cd "${BUILD_WORKSPACE_DIRECTORY:?Use bazel run}"' if ctx.attr.workspace else 'cd "$runfiles"')
    lines.append(ctx.attr.command)
    ctx.actions.write(launcher, "\n".join(lines) + "\n", is_executable = True)
    return [
        DefaultInfo(
            executable = launcher,
            runfiles = ctx.runfiles(files = ctx.files.srcs + ctx.files.prettier + ctx.files.buildifier + ctx.files.binary),
        ),
        RunEnvironmentInfo(inherited_environment = [
            "PATH",
            "HOME",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "CARGO_HOME",
        ]),
    ]

_attrs = {
    "binary": attr.label(doc = "Self-contained executable exposed as TRAIL_BINARY.", executable = True, cfg = "target"),
    "buildifier": attr.label(doc = "Pinned formatter exposed as TRAIL_BUILDIFIER.", allow_single_file = True),
    "command": attr.string(doc = "Bash command run from the declared input tree or workspace.", mandatory = True),
    "prettier": attr.label(doc = "Prettier entry module; include its package files in srcs.", allow_single_file = True),
    "srcs": attr.label_list(doc = "Files available in the command's runfiles tree.", allow_files = True),
    "workspace": attr.bool(doc = "Run in the editable checkout; only for explicit developer commands.", default = False),
}

local_command = rule(
    implementation = _command_impl,
    attrs = _attrs,
    executable = True,
    doc = "Declare an explicitly invoked host command with runfiles and optional workspace access.",
)
local_test = rule(
    implementation = _command_impl,
    attrs = _attrs,
    test = True,
    doc = "Declare a test over input runfiles; tag host-dependent checks external and no-remote.",
)
