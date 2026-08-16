import { job, unsafeShell, workflow } from "@dedalus-labs/hollywood";
import { checkoutAction } from "./actions";

const checkVouchedContributor = String.raw`set -euo pipefail

node <<'NODE'
const fs = require("node:fs");

const author = process.env.PR_AUTHOR;
const bootstrapMaintainers = process.env.VOUCH_BOOTSTRAP_MAINTAINERS;
if (!author) { console.error("PR_AUTHOR is required"); process.exit(1); }

const summaryPath = process.env.GITHUB_STEP_SUMMARY;
const authorHandle = author.toLowerCase();
const authorKey = "github:" + authorHandle;
const bootstrapSet = new Set(
  (bootstrapMaintainers || "").split(/[,\s]+/)
    .map(h => h.trim().toLowerCase().replace(/^@/, "").replace(/^github:/, ""))
    .filter(Boolean)
);

if (!fs.existsSync("VOUCHED.td")) {
  if (bootstrapSet.has(authorHandle)) {
    if (summaryPath) fs.appendFileSync(summaryPath, "## Vouch passed\n\n@" + author + " is a bootstrap maintainer.\n");
    console.log("@" + author + " is a bootstrap maintainer");
    process.exit(0);
  }
  if (summaryPath) fs.appendFileSync(summaryPath, "## Vouch blocked\n\nVOUCHED.td not present on trusted base.\n");
  console.error("VOUCHED.td not present on trusted base");
  process.exit(1);
}

const lines = fs.readFileSync("VOUCHED.td", "utf8").split("\n");
let vouched = false;
let denounced = null;

for (const rawLine of lines) {
  const line = rawLine.replace(/\r$/, "").trim();
  if (line === "" || line.startsWith("#")) continue;
  const [token, ...reasonParts] = line.split(/\s+/);
  const isDenounced = token.startsWith("-");
  const rawHandle = (isDenounced ? token.slice(1) : token).replace(/^@/, "");
  const handle = rawHandle.includes(":") ? rawHandle.toLowerCase() : "github:" + rawHandle.toLowerCase();
  if (handle !== authorKey) continue;
  if (isDenounced) { denounced = reasonParts.join(" ") || "no reason recorded"; break; }
  vouched = true;
}

if (denounced !== null) {
  if (summaryPath) fs.appendFileSync(summaryPath, "## Vouch blocked\n\n@" + author + " is denounced: " + denounced + "\n");
  console.error("@" + author + " is denounced: " + denounced);
  process.exit(1);
}
if (vouched) {
  if (summaryPath) fs.appendFileSync(summaryPath, "## Vouch passed\n\n@" + author + " is listed in VOUCHED.td.\n");
  console.log("@" + author + " is listed in VOUCHED.td");
  process.exit(0);
}

if (summaryPath) fs.appendFileSync(summaryPath, [
  "## Vouch required", "",
  "@" + author + " is not listed in VOUCHED.td.", "",
  "To get vouched:", "",
  "1. Open a \"Vouch request\" issue.",
  "2. Link public work or ask a vouched contributor to sponsor you.",
  "3. Wait for a maintainer to add your handle to VOUCHED.td.",
].join("\n") + "\n");
console.error("@" + author + " is not listed in VOUCHED.td");
process.exit(1);
NODE`;

export const vouch = workflow({
	name: "Vouch",
	on: {
		pull_request: {
			branches: ["main"],
			types: ["opened", "reopened", "synchronize", "ready_for_review"],
		},
	},
	permissions: { contents: "read" },
	jobs: {
		vouch: job({
			name: "Vouch Check",
			"runs-on": "ubuntu-latest",
			steps: [
				{
					name: "Checkout trusted base",
					uses: checkoutAction,
					with: {
						ref: "${{ github.event.pull_request.base.sha }}",
						"persist-credentials": false,
					},
				},
				{
					name: "Check contributor",
					env: {
						VOUCH_BOOTSTRAP_MAINTAINERS: "windsornguyen",
						PR_AUTHOR: "${{ github.event.pull_request.user.login }}",
					},
					run: unsafeShell(checkVouchedContributor),
				},
			],
		}),
	},
});
