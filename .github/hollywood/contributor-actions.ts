import { readFile } from "node:fs/promises";
import {
	action,
	stringInput,
	summaryCode,
	summaryText,
} from "@dedalus-labs/hollywood";

type VouchDecision = Readonly<{
	status: "blocked" | "passed";
	reason: string;
}>;

const normalizeHandle = (handle: string): string =>
	handle.trim().toLowerCase().replace(/^@/, "").replace(/^github:/, "");

const decideVouch = (
	author: string,
	bootstrapMaintainers: string,
	vouchedFile: string | undefined,
): VouchDecision => {
	const authorHandle = normalizeHandle(author);
	if (vouchedFile === undefined) {
		const bootstrap = new Set(
			bootstrapMaintainers.split(/[,\s]+/).map(normalizeHandle).filter(Boolean),
		);
		return bootstrap.has(authorHandle)
			? { status: "passed", reason: `@${author} is a bootstrap maintainer` }
			: { status: "blocked", reason: "VOUCHED.td not present on trusted base" };
	}

	const authorKey = `github:${authorHandle}`;
	let vouched = false;
	for (const rawLine of vouchedFile.split("\n")) {
		const line = rawLine.replace(/\r$/, "").trim();
		if (line === "" || line.startsWith("#")) continue;
		const [token, ...reasonParts] = line.split(/\s+/);
		const denounced = token.startsWith("-");
		const rawHandle = (denounced ? token.slice(1) : token).replace(/^@/, "");
		const handle = rawHandle.includes(":")
			? rawHandle.toLowerCase()
			: `github:${rawHandle.toLowerCase()}`;
		if (handle !== authorKey) continue;
		if (denounced) {
			const reason = reasonParts.join(" ") || "no reason recorded";
			return { status: "blocked", reason: `@${author} is denounced: ${reason}` };
		}
		vouched = true;
	}
	return vouched
		? { status: "passed", reason: `@${author} is listed in VOUCHED.td` }
		: { status: "blocked", reason: `@${author} is not listed in VOUCHED.td` };
};

const readVouchedFile = async (): Promise<string | undefined> => {
	try {
		return await readFile("VOUCHED.td", "utf8");
	} catch (error) {
		if (typeof error === "object" && error !== null && "code" in error && error.code === "ENOENT") {
			return undefined;
		}
		throw error;
	}
};

export const checkVouchedContributor = action({
	name: "Check vouched contributor",
	description: "Require a pull request author to be vouched on the trusted base.",
	localActionPath: "check-vouched-contributor",
	inputs: {
		author: stringInput({ description: "Pull request author." }),
		bootstrapMaintainers: stringInput({
			description: "Maintainers trusted before VOUCHED.td exists.",
			default: "",
		}),
	},
	outputs: {},
	run: async ({ input, log, summary }) => {
		const decision = decideVouch(
			input.author,
			input.bootstrapMaintainers,
			await readVouchedFile(),
		);
		await summary.table("Vouch", [
			{ label: "Contributor", value: summaryCode(`@${input.author}`) },
			{ label: "Decision", value: summaryText(decision.reason) },
		]);
		if (decision.status === "blocked") throw new Error(decision.reason);
		log.info(decision.reason);
		return {};
	},
});
