# CSS Library Integration Example

This example compiles Tailwind CSS and UnoCSS in-process and passes the resulting CSS into Takumi's `css` render option.

![Tailwind CSS output](./output/tailwind-stylesheets.png)
![UnoCSS output](./output/unocss-stylesheets.png)

## Run

Before running the example, build the native package once from the workspace root.

```bash
bun --filter '*' run build
```

Then render the example image.

```bash
cd example/css-library-integration
bun render
```

This writes:

- `output/tailwind.generated.css`
- `output/tailwind-stylesheets.png`
- `output/unocss.generated.css`
- `output/unocss-stylesheets.png`
