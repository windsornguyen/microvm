import { job, workflow } from "@dedalus-labs/hollywood";
import { actionlintAction, checkoutAction, rustToolchainAction } from "./actions";
import { trustedCiRun } from "./guards";

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
				{ name: "Format", run: "cargo fmt --check" },
				{ name: "Clippy", run: "cargo clippy --workspace -- -D warnings" },
				{ name: "Test", run: "cargo test --workspace" },
				{
					name: "Doc",
					run: "cargo doc --workspace --no-deps",
					env: { RUSTDOCFLAGS: "-D warnings" },
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
