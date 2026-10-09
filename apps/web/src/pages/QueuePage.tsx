import { OnlineEntry } from "./OnlineEntry";
import { QueuePanel } from "./QueuePanel";
export function QueuePage() {
  return <OnlineEntry mode="queue" Panel={QueuePanel} />;
}
