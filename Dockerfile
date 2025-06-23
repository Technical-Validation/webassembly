# Dockerfile 旨在为包含本地 Rust WebAssembly 依赖的 Next.js Web 应用创建容器镜像。
# 此 Dockerfile 针对 Next.js 的 'standalone' 输出模式进行了优化。
#
# 此 Dockerfile 假定构建上下文是包含 `web` 和 `wasm` 两个目录的项目根目录。
#
# 在项目根目录中执行以下命令进行构建:
# docker build -t my-next-app -f Dockerfile .
#

# --- 阶段 1: 构建 Rust WebAssembly 模块 ---
FROM rust:1.87.0 AS wasm-builder
# 安装 wasm-pack 用于构建 wasm
RUN cargo install wasm-pack
WORKDIR /app
# 复制 wasm 源代码并构建
COPY ./wasm ./wasm
RUN cd wasm && wasm-pack build --target web --out-dir pkg

# --- 阶段 2: 构建 Next.js standalone 应用 ---
FROM node:20-slim AS builder
WORKDIR /app

# 从 wasm-builder 阶段复制已构建的 wasm 包
COPY --from=wasm-builder /app/wasm/pkg ./wasm/pkg

# 复制 web 应用的包管理文件
COPY ./web/package.json ./web/yarn.lock* ./web/

# 切换到 web 目录并安装所有依赖
WORKDIR /app/web
RUN yarn install --frozen-lockfile

# 复制 web 应用的源代码并构建
# 这将生成 .next/standalone 和 .next/static 目录
COPY ./web .
RUN yarn build

# --- 阶段 3: 为 Next.js 应用创建生产环境镜像 ---
FROM node:20-slim AS runner
WORKDIR /app
ENV NODE_ENV production

# 为了安全，创建一个非 root 用户和组
RUN addgroup --system --gid 1001 nodejs
RUN adduser --system --uid 1001 nextjs

# 从 builder 阶段复制 standalone 输出和静态文件
# 这些是运行生产应用所需的唯一文件
COPY --from=builder --chown=nextjs:nodejs /app/web/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/web/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/web/.next/static ./.next/static

# 切换到非 root 用户
USER nextjs

# 暴露应用运行的端口
EXPOSE 3000
ENV PORT 3000

# 使用 node 直接启动 standalone server
CMD ["node", "server.js"]