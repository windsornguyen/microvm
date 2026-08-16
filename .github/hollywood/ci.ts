import { command, job, workflow } from "@dedalus-labs/hollywood";
import { actionlintAction, checkoutAction, rustToolchainAction } from "./actions";
import { trustedCiRun } from "./guards";

const maxBinaryBytes = 1024 * 1024;
const checkBinarySize = String.raw`
import { statSync } from "node:fs";

const path = process.argv[1];
const limit = Number(process.argv[2]);
const size = statSync(path).size;
if (size > limit) throw new Error(path + " is " + size + " bytes; limit is " + limit);
console.log(path + ": " + size + " bytes");
`;

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
				{
					name: "Check binary size",
					run: command({
						file: "node",
						args: [
							"--input-type=module",
							"--eval",
							checkBinarySize,
							"target/release/microvm",
							String(maxBinaryBytes),
						],
					}),
				},
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
