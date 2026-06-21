import { job, workflow } from "@dedalus-labs/hollywood";
import { checkoutAction, ghReleaseAction, rustToolchainAction } from "./actions";

export const binary = workflow({
	name: "Binary",
	on: {
		release: { types: ["published"] },
	},
	permissions: { contents: "write" },
	jobs: {
		build: job({
			name: "Build",
			"runs-on": "macos-15",
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				{ uses: rustToolchainAction },
				{ name: "Build", run: "cargo build --release" },
				{
					name: "Sign",
					run: "codesign --sign - --entitlements entitlements.plist --force target/release/microvm",
				},
				{
					name: "Package",
					run: [
						"mkdir -p dist",
						"cp target/release/microvm dist/",
						"cp entitlements.plist dist/",
						"cd dist && tar czf ../microvm-darwin-arm64.tar.gz .",
						"shasum -a 256 ../microvm-darwin-arm64.tar.gz | cut -d' ' -f1 > ../microvm-darwin-arm64.tar.gz.sha256",
					].join("\n"),
				},
				{
					name: "Upload",
					uses: ghReleaseAction,
					with: {
						files: [
							"microvm-darwin-arm64.tar.gz",
							"microvm-darwin-arm64.tar.gz.sha256",
						].join("\n"),
					},
				},
			],
		}),
	},
});
