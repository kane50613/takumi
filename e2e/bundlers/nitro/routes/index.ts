import { defineHandler } from "nitro/h3";
import { handle } from "../../src/app";

export default defineHandler(handle);
