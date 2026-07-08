// deno-fmt-ignore-file
// biome-ignore format: generated types do not need formatting
// prettier-ignore
import type { PathsForPages, GetConfigResponse, SearchCodecsForPages } from 'waku/router';

// prettier-ignore
import type { getConfig as File_ApiOgDocsSlugsImagePng_getConfig } from './pages/_api/og/docs/[...slugs]/image.png';
// prettier-ignore
import type { getConfig as File_Root_getConfig } from './pages/_root';
// prettier-ignore
import type { getConfig as File_DocsSlugs_getConfig } from './pages/docs/[...slugs]';
// prettier-ignore
import type { getConfig as File_Index_getConfig } from './pages/index';
// prettier-ignore
import type { getConfig as File_Playground_getConfig } from './pages/playground';
// prettier-ignore
import type { getConfig as File_Showcase_getConfig } from './pages/showcase';

// prettier-ignore
type Page =
| ({ path: '/_api/og/docs/[...slugs]/image.png' } & GetConfigResponse<typeof File_ApiOgDocsSlugsImagePng_getConfig>)
| ({ path: '/_root' } & GetConfigResponse<typeof File_Root_getConfig>)
| ({ path: '/docs/[...slugs]' } & GetConfigResponse<typeof File_DocsSlugs_getConfig>)
| ({ path: '/' } & GetConfigResponse<typeof File_Index_getConfig>)
| ({ path: '/playground' } & GetConfigResponse<typeof File_Playground_getConfig>)
| ({ path: '/showcase' } & GetConfigResponse<typeof File_Showcase_getConfig>);

// prettier-ignore
type Layout =
| { path: '/docs' };

// prettier-ignore
declare module 'waku/router' {
  interface RouteConfig {
    paths: PathsForPages<Page>;
  }
  interface CreatePagesConfig {
    pages: Page;
    layouts: Layout;
  }
  interface SearchCodecsConfig extends SearchCodecsForPages<Page> {}
}
