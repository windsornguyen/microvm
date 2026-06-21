import { job, workflow } from "@dedalus-labs/hollywood";
import { checkoutAction, releasePleaseAction, rustToolchainAction } from "./actions";

export const release = workflow({
	name: "Release",
	on: {
		push: { branches: ["main"] },
		workflow_dispatch: {},
	},
	permissions: { contents: "read" },
	jobs: {
		"release-please": job({
			name: "Release Please",
			"runs-on": "ubuntu-latest",
			permissions: {
				contents: "write",
				"pull-requests": "write",
			},
			outputs: {
				release_created: "${{ steps.release.outputs.release_created }}",
				tag_name: "${{ steps.release.outputs.tag_name }}",
			},
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				{
					id: "release",
					name: "Run release-please",
					uses: releasePleaseAction,
					with: {
						token: "${{ secrets.GITHUB_TOKEN }}",
						"config-file": "release-please-config.json",
						"manifest-file": ".release-please-manifest.json",
					},
				},
			],
		}),
		publish: job({
			name: "Publish to crates.io",
			needs: "release-please",
			if: "needs.release-please.outputs.release_created == 'true'",
			"runs-on": "ubuntu-latest",
			steps: [
				{ uses: checkoutAction, with: { "fetch-depth": 0 } },
				{ uses: rustToolchainAction },
				{
					name: "Publish microvm-vz",
					run: "cargo publish -p microvm-vz --no-verify",
					env: { CARGO_REGISTRY_TOKEN: "${{ secrets.CARGO_REGISTRY_TOKEN }}" },
				},
				{
					name: "Publish microvm",
					run: "cargo publish -p microvm --no-verify",
					env: { CARGO_REGISTRY_TOKEN: "${{ secrets.CARGO_REGISTRY_TOKEN }}" },
				},
			],
		}),
	},
});
