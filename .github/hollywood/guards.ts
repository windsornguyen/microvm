export const trustedCiRun =
	"github.repository == 'windsornguyen/microvm' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository)";
