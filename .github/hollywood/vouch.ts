import { job, uses, workflow } from "@dedalus-labs/hollywood";
import { checkoutAction } from "./actions";
import { checkVouchedContributor } from "./contributor-actions";

export const vouch = workflow({
	name: "Vouch",
	on: {
		pull_request: {
			types: ["opened", "reopened", "synchronize", "ready_for_review"],
		},
	},
	permissions: { contents: "read" },
	jobs: {
		vouch: job({
			name: "Vouch Check",
			"runs-on": "ubuntu-latest",
			steps: [
				{
					name: "Checkout trusted base",
					uses: checkoutAction,
					with: {
						ref: "${{ github.event.pull_request.base.sha }}",
						"persist-credentials": false,
					},
				},
				uses(checkVouchedContributor, {
					with: {
						author: "${{ github.event.pull_request.user.login }}",
						bootstrapMaintainers: "windsornguyen",
					},
				}),
			],
		}),
	},
});
