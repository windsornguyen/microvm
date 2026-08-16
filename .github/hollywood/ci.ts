import { command, job, uses, workflow } from "@dedalus-labs/hollywood";
import { actionlintAction, checkoutAction, rustToolchainAction } from "./actions";
import { checkFileSize } from "./artifact-actions";
import { trustedCiRun } from "./guards";

const maxBinaryBytes = 1024 * 1024;

export const ci = workflow({
	name: "CI",
	on: {
		push: { branches: ["main"] },
		pull_request: { branches: ["main"] },
	},
	permissions: { contents: "read" },
	jobs: {
		check: job({
			name: "Check",
			if: trustedCiRun,
			"runs-on": "macos-15",
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
		actionlint: job({
			name: "Actionlint",
			if: trustedCiRun,
			"runs-on": "ubuntu-latest",
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				{ uses: actionlintAction },
			],
		}),
	},
});
