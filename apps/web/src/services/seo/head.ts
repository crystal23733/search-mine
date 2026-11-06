import type { DocumentMetadata } from "./document";
export function applyDocumentHead(
  document: Document,
  value: DocumentMetadata,
): () => void {
  const title = document.title,
    language = document.documentElement.lang;
  const previous = [...document.head.querySelectorAll("[data-liar-seo]")];
  for (const node of previous) node.remove();
  const nodes: HTMLElement[] = [];
  const add = (tag: "meta" | "link", attributes: Record<string, string>) => {
    const node = document.createElement(tag);
    node.setAttribute("data-liar-seo", "");
    for (const [key, content] of Object.entries(attributes))
      node.setAttribute(key, content);
    document.head.append(node);
    nodes.push(node);
  };
  document.title = value.title;
  document.documentElement.lang = value.locale;
  if (value.noindex)
    add("meta", { name: "robots", content: "noindex, noarchive" });
  if (value.description)
    add("meta", { name: "description", content: value.description });
  if (value.canonical) {
    add("link", { rel: "canonical", href: value.canonical });
    for (const alternate of value.alternates)
      add("link", {
        rel: "alternate",
        hreflang: alternate.language,
        href: alternate.href,
      });
    add("meta", { property: "og:type", content: "website" });
    add("meta", { property: "og:title", content: value.title });
    add("meta", { property: "og:description", content: value.description! });
    add("meta", { property: "og:url", content: value.canonical });
  }
  return () => {
    for (const node of nodes) node.remove();
    for (const node of previous) document.head.append(node);
    document.title = title;
    document.documentElement.lang = language;
  };
}
