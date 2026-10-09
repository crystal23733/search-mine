import { OnlineEntry } from "./OnlineEntry";
import { FriendsPanel } from "./FriendsPanel";
export function FriendsPage() {
  return <OnlineEntry mode="friends" Panel={FriendsPanel} />;
}
