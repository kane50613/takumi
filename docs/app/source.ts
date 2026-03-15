import { loader } from "fumadocs-core/source";
import defaultAttributes from "lucide-react/dist/esm/defaultAttributes";
import { __iconNode as arrowBigRightIconNode } from "lucide-react/dist/esm/icons/arrow-big-right";
import { __iconNode as axeIconNode } from "lucide-react/dist/esm/icons/axe";
import { __iconNode as bookMarkedIconNode } from "lucide-react/dist/esm/icons/book-marked";
import { __iconNode as brainIconNode } from "lucide-react/dist/esm/icons/brain";
import { __iconNode as bugIconNode } from "lucide-react/dist/esm/icons/bug";
import { __iconNode as circleFadingArrowUpIconNode } from "lucide-react/dist/esm/icons/circle-fading-arrow-up";
import { __iconNode as imageMarkedIconNode } from "lucide-react/dist/esm/icons/image";
import { __iconNode as layersIconNode } from "lucide-react/dist/esm/icons/layers";
import { __iconNode as layoutTemplateIconNode } from "lucide-react/dist/esm/icons/layout-template";
import { __iconNode as leafIconNode } from "lucide-react/dist/esm/icons/leaf";
import { __iconNode as playIconNode } from "lucide-react/dist/esm/icons/play";
import { __iconNode as rulerIconNode } from "lucide-react/dist/esm/icons/ruler";
import { __iconNode as toyBrickIconNode } from "lucide-react/dist/esm/icons/toy-brick";
import { __iconNode as typeIconNode } from "lucide-react/dist/esm/icons/type";
import { __iconNode as windIconNode } from "lucide-react/dist/esm/icons/wind";
import { __iconNode as zapIconNode } from "lucide-react/dist/esm/icons/zap";
import { createElement } from "react";
import { docs } from "../.source/server";

const iconProps = {
  ...defaultAttributes,
  color: "currentColor",
  strokeWidth: 2,
  width: 24,
  height: 24,
};

const icons = {
  Leaf: leafIconNode,
  Brain: brainIconNode,
  Play: playIconNode,
  Ruler: rulerIconNode,
  ToyBrick: toyBrickIconNode,
  Axe: axeIconNode,
  ArrowBigRight: arrowBigRightIconNode,
  Bug: bugIconNode,
  BookMarked: bookMarkedIconNode,
  Image: imageMarkedIconNode,
  Type: typeIconNode,
  Layers: layersIconNode,
  Zap: zapIconNode,
  CircleFadingArrowUp: circleFadingArrowUpIconNode,
  LayoutTemplate: layoutTemplateIconNode,
  Wind: windIconNode,
};

export const source = loader({
  icon(icon) {
    if (icon && icon in icons) {
      return createElement(
        "svg",
        iconProps,
        icons[icon as keyof typeof icons].map(([tag, attrs]) =>
          createElement(tag, attrs),
        ),
      );
    }
  },
  source: docs.toFumadocsSource(),
  baseUrl: "/docs",
});
