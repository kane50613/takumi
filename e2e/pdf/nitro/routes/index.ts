import { defineHandler } from "nitro/h3";
import { handle } from "../../src/pdf";

export default defineHandler(handle);
