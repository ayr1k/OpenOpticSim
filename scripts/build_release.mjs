// Windows release build with portable diagnostic paths; never uploads or signs.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.platform !== "win32" || process.arch !== "x64") {
  throw new Error("The installer build is validated on Windows x64 only.");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
if (process.env.RUSTFLAGS && !process.env.CARGO_ENCODED_RUSTFLAGS) {
  throw new Error("Custom RUSTFLAGS detected. Express them as CARGO_ENCODED_RUSTFLAGS before this release recipe, so argument quoting is preserved.");
}
const flags = process.env.CARGO_ENCODED_RUSTFLAGS
  ? process.env.CARGO_ENCODED_RUSTFLAGS.split("\x1f")
  : [];
// rustc uses textual matching, so supply both Windows separator conventions.
// Broad home mapping comes first; the more specific mappings take precedence.
for (const [from, to] of [
  [homedir(), "/build/home"],
  [process.env.RUSTUP_HOME || resolve(homedir(), ".rustup"), "/build/rustup"],
  [process.env.CARGO_HOME || resolve(homedir(), ".cargo"), "/build/cargo"],
  [root, "/src/OpenOpticSim"],
]) {
  for (const prefix of new Set([resolve(from), resolve(from).replaceAll("\\", "/")])) {
    flags.push(`--remap-path-prefix=${prefix}=${to}`);
  }
}
// MSVC writes its PDB path separately from rustc. Keep the file name only.
flags.push("-C", "link-arg=/PDBALTPATH:%_PDB%");
const env = { ...process.env, CARGO_ENCODED_RUSTFLAGS: flags.join("\x1f") };

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run("cargo", ["test", "--workspace", "--locked", "--release"]);
run("cargo", ["build", "-p", "oos-cli", "--locked", "--release"]);
run("cargo", ["test", "--manifest-path", "OOS-Wollaston/Cargo.toml", "--workspace", "--locked", "--release"]);
run("cargo", ["build", "--manifest-path", "OOS-Wollaston/Cargo.toml", "--workspace", "--locked", "--release"]);
// Static command only: no paths or user input are interpolated into the shell.
run(process.env.ComSpec || "cmd.exe", ["/d", "/s", "/c", "pnpm tauri build --bundles nsis"]);
for (const name of ["openopticsim-desktop.exe", "oos-cli.exe", "../../OOS-Wollaston/target/release/wollaston_ffi.dll", "../../OOS-Wollaston/target/release/wollaston-cli.exe"]) {
  const data = readFileSync(resolve(root, "target/release", name));
  const ascii = data.toString("latin1").match(/[A-Z]:[\\/]+Users[\\/]+[^\\/\0\r\n]+/gi) || [];
  const utf16 = data.toString("utf16le").match(/[A-Z]:[\\/]+Users[\\/]+[^\\/\0\r\n]+/gi) || [];
  console.log(`${name}: ${ascii.length + utf16.length} personal-home path matches`);
  if (ascii.length || utf16.length) throw new Error("Review embedded paths before distributing the binary; no matched values are printed.");
}
