import {
	action,
	job,
	stringInput,
	stringOutput,
	uses,
	workflow,
	type ScriptExec,
	type ScriptLog,
} from "@dedalus-labs/hollywood";
import { checkoutAction, releasePleaseAction, rustToolchainAction } from "./actions";

const CRATES = ["microvm-vz", "microvm"] as const;
const MAX_ATTEMPTS = 3;
const BASE_DELAY_MS = 5_000;
const MAX_DELAY_MS = 15_000;

const publishCrate = async (
	exec: ScriptExec,
	log: ScriptLog,
	crate: string,
): Promise<void> => {
	for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
		try {
			await exec("cargo", ["publish", "-p", crate, "--no-verify"]);
			log.info(`published ${crate}`);
			return;
		} catch (err) {
			if (attempt === MAX_ATTEMPTS) {
				throw new Error(
					`failed to publish ${crate} after ${MAX_ATTEMPTS} attempts: ${err}`,
				);
			}
			const delay = Math.min(BASE_DELAY_MS * 2 ** (attempt - 1), MAX_DELAY_MS);
			log.info(
				`publish ${crate} attempt ${attempt}/${MAX_ATTEMPTS} failed, retrying in ${delay}ms`,
			);
			await new Promise((resolve) => setTimeout(resolve, delay));
		}
	}
};

export const publishCrates = action({
	name: "Publish crates to crates.io",
	description:
		"Publish workspace crates in dependency order with bounded exponential backoff.",
	localActionPath: "publish-crates",
	inputs: {
		token: stringInput({ description: "crates.io registry token." }),
	},
	outputs: {
		published: stringOutput({
			description: "Comma-separated list of published crates.",
		}),
	},
	run: async ({ exec, input, log }) => {
		const published: string[] = [];
		for (const crate of CRATES) {
			await publishCrate(
				(cmd, args, opts) =>
					exec(cmd, args, {
						...opts,
						env: { ...opts?.env, CARGO_REGISTRY_TOKEN: input.token },
					}),
				log,
				crate,
			);
			published.push(crate);
		}
		return { published: published.join(",") };
	},
});

export const release = workflow({
	name: "Release",
	on: {
		push: { branches: ["main"] },
		workflow_dispatch: {},
	},
	permissions: { contents: "read" },
	env: {
		FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true",
	},
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
				{
					uses: checkoutAction,
					with: { "persist-credentials": false },
				},
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
				uses(publishCrates, {
					with: { token: "${{ secrets.CARGO_REGISTRY_TOKEN }}" },
				}),
			],
		}),
	},
});
