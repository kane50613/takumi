import * as emoji from "@takumi-rs/helpers/emoji";

/** @deprecated Import `EmojiType` from `@takumi-rs/helpers/emoji` instead. Will be removed in v3. */
export type EmojiType = emoji.EmojiType;

/** @deprecated Use the `emoji` render option instead. Will be removed in v3. */
export const extractEmojis: typeof emoji.extractEmojis = emoji.extractEmojis;

/** @deprecated Import `isEmojiImageUrl` from `@takumi-rs/helpers/emoji` instead. Will be removed in v3. */
export const isEmojiImageUrl: typeof emoji.isEmojiImageUrl = emoji.isEmojiImageUrl;
