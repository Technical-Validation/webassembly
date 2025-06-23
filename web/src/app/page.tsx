"use client";
import * as wasm from "my_wasm_template";

export default function Home() {
  const tools = [
    {
      name: "wasm 调用浏览器 api",
      description: 'wasm.greet("browser") => alert("")',
      action: async () => {
        await wasm.default();
        wasm.greet("browser");
      },
    },
    {
      name: "直接返回数据",
      description: "wasm.add(1, 2) => 3",
      action: async () => {
        await wasm.default();
        alert(wasm.add(1, 2));
      },
    },
    {
      name: "json 数据解析",
      description:
        "{\n" +
        '          name: "John",\n' +
        "          age: 30,\n" +
        '          city: "New York",\n' +
        " }",
      action: async () => {
        await wasm.default();
        const json = {
          name: "John",
          age: 30,
          city: "New York",
        };
        const result = wasm.json_reverse(JSON.stringify(json));
        alert(result);
        console.log("json reverse result:", JSON.parse(result));
      },
    },
  ];

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-8 flex flex-col items-center justify-center">
      <div className="max-w-7xl mx-auto text-cente w-full">
        <h1 className="text-6xl font-bold text-gray-800 mb-16">
          WebAssembly 示例
        </h1>
        <div className="grid grid-cols-2 gap-8">
          {tools.map((tool, index) => (
            <div
              key={index}
              className="rounded-3xl relative bg-zinc-700 p-10 shadow-2xl hover:shadow-3xl transition-all duration-300 transform hover:-translate-y-2 border border-white/10 backdrop-blur-sm"
            >
              <h2 className="text-4xl font-bold text-white mb-2 whitespace-pre-wrap">
                {tool.name}
              </h2>
              <p className="text-xl text-white whitespace-pre-wrap">
                {tool.description}
              </p>
              <button
                className="mt-8 px-8 py-4 cursor-pointer bg-white bg-opacity-20 rounded-full text-xl font-semibold hover:bg-opacity-30 transition hover:scale-105"
                onClick={tool.action}
              >
                测试
              </button>
              <div className="absolute bottom-4 right-4 text-8xl font-bold text-white/10 z-0">
                {index + 1}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
