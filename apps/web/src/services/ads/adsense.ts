import type { AutoAdsScriptPort } from "./types";
const SOURCE = "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js";
export function createAdSenseScript(
  document: Document,
  publisherId?: unknown,
): AutoAdsScriptPort {
  const client =
    typeof publisherId === "string" && /^ca-pub-\d{16}$/.test(publisherId)
      ? publisherId
      : null;
  let attempted = false;
  return {
    configured: () => client !== null,
    load(signal) {
      if (!client || attempted || signal.aborted)
        return Promise.reject(new Error("unavailable"));
      attempted = true;
      return new Promise<void>((resolve, reject) => {
        // A removed tag does not unload an SDK; the caller owns the document barrier.
        const node = document.createElement("script");
        let timer: ReturnType<typeof setTimeout>;
        const finish = (success: boolean) => {
          clearTimeout(timer);
          signal.removeEventListener("abort", abort);
          node.removeEventListener("load", loaded);
          node.removeEventListener("error", failed);
          if (success) resolve();
          else {
            node.remove();
            reject(new Error("unavailable"));
          }
        };
        const loaded = () => finish(true);
        const failed = () => finish(false);
        const abort = () => finish(false);
        timer = setTimeout(failed, 3000);
        signal.addEventListener("abort", abort, { once: true });
        node.addEventListener("load", loaded, { once: true });
        node.addEventListener("error", failed, { once: true });
        node.async = true;
        node.crossOrigin = "anonymous";
        node.dataset.liarAutoAds = "adsense";
        node.src = `${SOURCE}?client=${client}`;
        try {
          if (document.querySelector("script[data-liar-auto-ads]"))
            throw new Error("unavailable");
          document.head.append(node);
        } catch {
          finish(false);
        }
      });
    },
  };
}
