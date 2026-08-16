import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { stat, writeFile } from "node:fs/promises";
import { action, integerInput, pathInput } from "@dedalus-labs/hollywood";

export const checksumArchive = action({
	name: "Checksum archive",
	description: "Write the SHA-256 digest for a release archive.",
	localActionPath: "checksum-archive",
	inputs: {
		archive: pathInput({ description: "Release archive to checksum." }),
	},
	outputs: {},
	run: async ({ input, log }) => {
		const hash = createHash("sha256");
		for await (const chunk of createReadStream(input.archive)) hash.update(chunk);
		const digest = hash.digest("hex");
		const checksum = `${input.archive}.sha256`;
		await writeFile(checksum, `${digest}\n`, "ascii");
		log.info(`${checksum}: ${digest}`);
		return {};
	},
});

export const checkFileSize = action({
	name: "Check file size",
	description: "Require a file to fit within a byte limit.",
	localActionPath: "check-file-size",
	inputs: {
		file: pathInput({ description: "File to measure." }),
		maxBytes: integerInput({ description: "Maximum permitted size in bytes." }),
	},
	outputs: {},
	run: async ({ input, log }) => {
		const size = (await stat(input.file)).size;
		if (size > input.maxBytes) {
			throw new Error(`${input.file} is ${size} bytes; limit is ${input.maxBytes}`);
		}
		log.info(`${input.file}: ${size} bytes`);
		return {};
	},
});
