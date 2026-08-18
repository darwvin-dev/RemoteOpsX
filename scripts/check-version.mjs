import fs from "node:fs";

const pkg = JSON.parse(fs.readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const tauri = JSON.parse(
  fs.readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
const cargo = fs.readFileSync(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");

const packageMarker = "[package]";
const packageStart = cargo.indexOf(packageMarker);
const afterPackage = packageStart >= 0 ? cargo.slice(packageStart + packageMarker.length) : "";
const nextSection = afterPackage.search(/^\[[^\n]+\]/m);
const packageSection = nextSection >= 0 ? afterPackage.slice(0, nextSection) : afterPackage;
const cargoVersion = packageSection.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

const versions = {
  "package.json": pkg.version,
  "src-tauri/Cargo.toml": cargoVersion,
  "src-tauri/tauri.conf.json": tauri.version,
};

if (Object.values(versions).some((value) => !value)) {
  console.error("Unable to read all RemoteOpsX versions:", versions);
  process.exit(1);
}

const unique = new Set(Object.values(versions));
if (unique.size !== 1) {
  console.error("RemoteOpsX version mismatch:");
  for (const [file, version] of Object.entries(versions)) {
    console.error(`  ${file}: ${version}`);
  }
  process.exit(1);
}

const version = pkg.version;
const allowed = /^\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)(?:\.\d+)?)?$/;
if (!allowed.test(version)) {
  console.error(
    `Unsupported version format ${version}. Use X.Y.Z, X.Y.Z-alpha.N, X.Y.Z-beta.N or X.Y.Z-rc.N.`,
  );
  process.exit(1);
}

const releaseTag = process.env.REMOTEOPSX_RELEASE_TAG?.trim();
if (releaseTag && releaseTag !== `v${version}`) {
  console.error(`Release tag ${releaseTag} does not match project version v${version}.`);
  process.exit(1);
}

console.log(`RemoteOpsX version OK: ${version}`);
