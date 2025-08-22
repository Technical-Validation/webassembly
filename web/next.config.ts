import type { NextConfig } from "next";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";
import path from "node:path";

const nextConfig: NextConfig = {
  /* config options here */
  productionBrowserSourceMaps: false,
  output: "standalone",

  webpack(config) {
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true,
      layers: true,
    };

    if (process.env.NODE_ENV === "development") {
      config.plugins.push(
        new WasmPackPlugin({
          // `crateDirectory` 指向包含 `Cargo.toml` 文件的目录
          crateDirectory: path.resolve(__dirname, "../wasm"),
          // 明确指定输出目标
          outDir: path.resolve(__dirname, "../wasm/pkg"),
          // 强制在开发模式下也运行 wasm-pack
          forceMode: "development",
        }),
      );
    }
    return config;
  },
};

export default nextConfig;
