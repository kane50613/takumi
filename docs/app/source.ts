import { loader } from "fumadocs-core/source";
import {
  Axe,
  Brain,
  Bug,
  Cog,
  FileCode,
  Film,
  Hand,
  Image,
  Layers,
  Leaf,
  Palette,
  Play,
  Ruler,
  Shovel,
  Smile,
  Sparkles,
  ToyBrick,
  Type,
  Wind,
  Wrench,
  Zap,
} from "lucide-react";
import { createElement } from "react";
import { docs } from "../.source/server";

const icons = {
  Axe,
  Brain,
  Bug,
  Cog,
  FileCode,
  Film,
  Hand,
  Image,
  Layers,
  Leaf,
  Palette,
  Play,
  Ruler,
  Shovel,
  Smile,
  Sparkles,
  ToyBrick,
  Type,
  Wind,
  Wrench,
  Zap,
};

export const source = loader({
  source: docs.toFumadocsSource(),
  baseUrl: "/docs",
  icon(name) {
    if (name && name in icons) {
      return createElement(icons[name as keyof typeof icons]);
    }
  },
});
