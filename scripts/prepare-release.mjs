import { execFileSync } from "node:child_process";
import path from "node:path";

const nextVersion = process.argv[2]?.trim();

if (!nextVersion) {
  console.error("Usage: npm run release:prepare -- <version>");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+$/.test(nextVersion)) {
  console.error(`Invalid version "${nextVersion}". Expected semver like 0.1.1`);
  process.exit(1);
}

const rootDir = process.cwd();
const bumpScriptPath = path.join(rootDir, "scripts", "bump-version.mjs");
const tagName = `v${nextVersion}`;

execFileSync("node", [bumpScriptPath, nextVersion], {
  cwd: rootDir,
  stdio: "inherit",
});

let insideGit = false;

try {
  const result = execFileSync("git", ["rev-parse", "--is-inside-work-tree"], {
    cwd: rootDir,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  insideGit = result.trim() === "true";
} catch {
  insideGit = false;
}

if (!insideGit) {
  console.log("");
  console.log("Version updated, but no Git repository was detected here.");
  console.log(`When ready, create a tag manually: git tag ${tagName}`);
  process.exit(0);
}

try {
  execFileSync("git", ["rev-parse", "--verify", "--quiet", tagName], {
    cwd: rootDir,
    stdio: "ignore",
  });
  console.error(`Git tag ${tagName} already exists.`);
  process.exit(1);
} catch {
  // Tag does not exist, continue.
}

execFileSync("git", ["tag", tagName], {
  cwd: rootDir,
  stdio: "inherit",
});

console.log("");
console.log(`Release prepared for ${nextVersion}`);
console.log(`Next step: git push origin ${tagName}`);
