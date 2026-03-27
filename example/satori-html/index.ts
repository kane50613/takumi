import { write } from "bun";
import { html } from "satori-html";
import { render } from "takumi-js";

const markup = html` <div style="color: black">hello, world</div> `;

const png = await render(markup, {
  width: 600,
  height: 400,
});

await write("./output.png", png.buffer);
