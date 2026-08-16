import {
	GitHubJobResult,
	always,
	and,
	command,
	format,
	github,
	job,
	needsResultIs,
	not,
	uses,
	workflow,
} from "@dedalus-labs/hollywood";
import { actionlintAction, checkoutAction, rustToolchainAction } from "./actions";
import { checkFileSize } from "./artifact-actions";

const maxBinaryBytes = 1024 * 1024;

const requiredChecksPassed = and(
	needsResultIs("rust", GitHubJobResult.Success),
	needsResultIs("workflows", GitHubJobResult.Success),
);

export const ci = workflow({
	name: "CI",
	on: {
		push: { branches: ["main"] },
		pull_request: {},
		workflow_dispatch: {},
	},
	permissions: {},
	concurrency: {
		group: format("{0}-{1}", github.workflow, github.ref),
		"cancel-in-progress": true,
	},
	jobs: {
		rust: job({
			name: "Rust",
			"runs-on": "macos-15",
			"timeout-minutes": 30,
			permissions: { contents: "read" },
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				{ uses: rustToolchainAction, with: { components: "rustfmt, clippy" } },
				{
					name: "Format",
					run: command({ file: "cargo", args: ["fmt", "--check"] }),
				},
				{
					name: "Clippy",
					run: command({
						file: "cargo",
						args: ["clippy", "--workspace", "--", "-D", "warnings"],
					}),
				},
				{
					name: "Test",
					run: command({ file: "cargo", args: ["test", "--workspace"] }),
				},
				{
					name: "Doc",
					run: command({
						file: "cargo",
						args: ["doc", "--workspace", "--no-deps"],
					}),
					env: { RUSTDOCFLAGS: "-D warnings" },
				},
				{
					name: "Build release",
					run: command({ file: "cargo", args: ["build", "--release"] }),
				},
				uses(checkFileSize, {
					with: {
						file: "target/release/microvm",
						maxBytes: String(maxBinaryBytes),
					},
				}),
			],
		}),
		workflows: job({
			name: "Generated workflows",
			"runs-on": "ubuntu-latest",
			"timeout-minutes": 10,
			permissions: { contents: "read" },
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				{
					name: "Install dependencies",
					run: command({ file: "npm", args: ["ci"] }),
				},
				{
					name: "Regenerate workflows",
					run: command({ file: "npm", args: ["run", "generate"] }),
				},
				{
					name: "Regenerate actions",
					run: command({ file: "npm", args: ["run", "actions"] }),
				},
				{
					name: "Require clean generated output",
					run: command({
						file: "git",
						args: [
							"diff",
							"--exit-code",
							"--",
							".github/actions",
							".github/workflows",
						],
					}),
				},
				{ uses: actionlintAction },
			],
		}),
		ci: job({
			name: "ci-allgreen",
			if: always(),
			needs: ["rust", "workflows"],
			"runs-on": "ubuntu-latest",
			"timeout-minutes": 5,
			permissions: {},
			steps: [
				{
					name: "Accept complete CI",
					if: requiredChecksPassed,
					run: command({ file: "true", args: [] }),
				},
				{
					name: "Reject incomplete CI",
					if: not(requiredChecksPassed),
					run: command({ file: "false", args: [] }),
				},
			],
		}),
	},
});
