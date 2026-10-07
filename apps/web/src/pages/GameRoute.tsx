import { useEffect, useState } from "preact/hooks";
import type { ComponentType } from "preact";
import { useUi } from "../ui/context";
export function GameRoute({ route }: { route: "practice" | "tutorial" }) {
  const { t } = useUi();
  const [loaded, setLoaded] = useState<{
    route: string;
    Page?: ComponentType;
    error?: boolean;
  }>();
  useEffect(() => {
    let current = true;
    const loading =
      route === "practice"
        ? import("./MatchPage").then((module) => module.MatchPage)
        : import("./TutorialPage").then((module) => module.TutorialPage);
    void loading
      .then((Page) => {
        if (current) setLoaded({ route, Page });
      })
      .catch(() => {
        if (current) setLoaded({ route, error: true });
      });
    return () => {
      current = false;
    };
  }, [route]);
  if (loaded?.route === route && loaded.Page) return <loaded.Page />;
  return (
    <>
      <h1>{t(route === "practice" ? "match.title" : "tutorial.title")}</h1>
      <p role={loaded?.error ? "alert" : "status"}>
        {t(loaded?.error ? "match.unavailable" : "match.loading")}
      </p>
    </>
  );
}
