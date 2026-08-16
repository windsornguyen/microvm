import { command, job, uses, workflow } from "@dedalus-labs/hollywood";
import { checkoutAction, ghReleaseAction, rustToolchainAction } from "./actions";
import { checksumArchive } from "./artifact-actions";

const archive = "microvm-darwin-arm64.tar.gz";

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
				{
					name: "Build",
					run: command({ file: "cargo", args: ["build", "--release"] }),
				},
				{
					name: "Sign",
					run: command({
						file: "codesign",
						args: [
							"--sign",
							"-",
							"--entitlements",
							"entitlements.plist",
							"--force",
							"target/release/microvm",
						],
					}),
				},
				{
					name: "Prepare package",
					run: command({ file: "mkdir", args: ["-p", "dist"] }),
				},
				{
					name: "Copy binary",
					run: command({
						file: "cp",
						args: ["target/release/microvm", "dist/"],
					}),
				},
				{
					name: "Copy entitlements",
					run: command({
						file: "cp",
						args: ["entitlements.plist", "dist/"],
					}),
				},
				{
					name: "Archive",
					run: command({
						file: "tar",
						args: [
							"-C",
							"dist",
							"-czf",
							`../${archive}`,
							".",
						],
					}),
				},
				{
					name: "Checksum",
					...uses(checksumArchive, { with: { archive } }),
				},
				{
					name: "Upload",
					uses: ghReleaseAction,
					with: {
						files: [archive, `${archive}.sha256`].join("\n"),
					},
				},
			],
		}),
	},
});
