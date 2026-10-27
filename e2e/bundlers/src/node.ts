import { handle } from "./app";

const response = await handle();

console.log(
  `magic ${Buffer.from(await response.arrayBuffer())
    .subarray(0, 4)
    .toString("hex")}`,
);
