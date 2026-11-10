import { writeFileSync } from 'node:fs';
import { LOCALES } from '../apps/web/src/services/locale.ts';
import { DOCUMENT_ROUTES } from '../apps/web/src/services/document-route.ts';
import { PUBLIC_DOCUMENTS, publicOrigin } from '../apps/web/src/services/seo/document.ts';

const origin = publicOrigin(process.env.VITE_PUBLIC_SITE_ORIGIN);
const paths = ['', ...LOCALES.map(locale => `/${locale}`)];
const literal = path => {
  if (!/^\/[a-zA-Z0-9/-]*$/.test(path)) throw Error('Invalid document path');
  return `"${path}"`;
};
const publicFiles = LOCALES.flatMap(locale => PUBLIC_DOCUMENTS.map(route => {
  const path = `/${locale}${route}`;
  return `    ${literal(path)} "/${locale}${route === '/' ? '/index' : route}.html";`;
}));
const appPaths = paths.flatMap(prefix => DOCUMENT_ROUTES.map(route => `${prefix}${route}`));
const errors = paths.map(prefix => `    ${literal(`${prefix}/403`)} 403;`);
const ws = origin ? ` ${origin.replace(/^https:/, 'wss:')}` : '';
const config = `# Generated from the application's locale/document lists; do not edit.
map $request_uri $document_file {
    default /index.html;
${publicFiles.join('\n')}
}
map $uri $document_status {
    default 404;
${appPaths.map(path => `    ${literal(path)} 200;`).join('\n')}
${errors.join('\n')}
}
map "$status:$document_file" $document_cache {
    default "no-store";
${LOCALES.flatMap(locale => PUBLIC_DOCUMENTS.map(route => `    "200:/${locale}${route === '/' ? '/index' : route}.html" "no-cache";`)).join('\n')}
}
map "$status:$document_file" $document_robots {
    default "noindex, noarchive";
${LOCALES.flatMap(locale => PUBLIC_DOCUMENTS.map(route => `    "200:/${locale}${route === '/' ? '/index' : route}.html" "";`)).join('\n')}
}
map "$status:$uri" $response_cache {
    default $document_cache;
    ~^200:/assets/[a-zA-Z0-9_/-]+-[a-zA-Z0-9_-]+\\.(?:js|css|wasm)$ "public, max-age=31536000, immutable";
    ~^200:/(?:core/|icons/|manifest\\.webmanifest|service-worker\\.js|robots\\.txt|sitemap\\.xml) "no-cache";
    ~^[0-9]+:/(?:api/|health/) "no-store";
}
map $http_upgrade $connection_upgrade {
    default close;
    websocket upgrade;
}
map $uri $content_security_policy {
    default "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; worker-src 'self' blob:; connect-src 'self'${ws}; manifest-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'";
}
`;
writeFileSync(process.argv[2] ?? 'deploy/routes.generated.conf', config);
