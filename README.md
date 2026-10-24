<div align="center">
  <img src="./apps/app/icons/128x128.png" width="128" height="128" alt="Axolotl Launcher Logo" />
  <h1>Axolotl Launcher</h1>
  <p><strong>次世代 Minecraft 桌面客户端，全能、美观、全平台覆盖。</strong></p>

  <p>
    <a href="https://github.com/Mystic-Stars/Axolotl/actions">
      <img src="https://img.shields.io/github/actions/workflow/status/Mystic-Stars/Axolotl/axolotl-ci.yml?style=for-the-badge&logo=github" alt="Desktop CI" />
    </a>
    <a href="https://github.com/Mystic-Stars/Axolotl/releases">
      <img src="https://img.shields.io/github/downloads/Mystic-Stars/Axolotl/total?style=for-the-badge&logo=github" alt="Downloads" />
    </a>
    <a href="https://github.com/Mystic-Stars/Axolotl/stargazers">
      <img src="https://img.shields.io/github/stars/Mystic-Stars/Axolotl?style=for-the-badge&logo=github&color=ffb800" alt="Stars" />
    </a>
    <a href="COPYING.md">
      <img src="https://img.shields.io/badge/License-GPL_3.0-blue.svg?style=for-the-badge" alt="License" />
    </a>
  </p>

  <p>
    <a href="https://www.axlmc.org">官方网站</a> ｜
    <a href="https://github.com/Mystic-Stars/Axolotl/releases/latest">下载最新版</a> ｜
    <a href="CONTRIBUTING.md">参与贡献</a>
    ｜ <a href="CODE_OF_CONDUCT.md">行为准则</a>
  </p>
</div>

<details open>
<summary><strong>赞助与合作</strong></summary>

感谢以下赞助商与合作伙伴对 Axolotl Launcher 的支持。

<table>
  <tr>
    <td><img src="./.github/assets/codeflow-logo.png" width="72" alt="CodeFlow Logo" /></td>
    <td><strong>CodeFlow</strong><br>更稳、更省地调用顶级 AI 模型<br>原生协议转发 · 无需海外网络 · 支付宝即充即用</td>
    <td><a href="https://codeflow.sh/register?invite=4UHP2KYH">访问 CodeFlow</a></td>
  </tr>
</table>

<a href="https://www.netlify.com">
  <img src="https://www.netlify.com/assets/badges/netlify-badge-color-accent.svg" alt="Deploys by Netlify" />
</a>

- [在爱发电支持 Axolotl Launcher](https://ifdian.net/a/Mystic-Stars) — 你的支持将帮助项目持续维护与改进

</details>

---

**Axolotl Launcher（美西螈启动器）** 是一款免费、开源、跨平台的 Minecraft Java 版第三方启动器，支持在一个客户端中搜索、安装和更新来自 Modrinth 与 CurseForge 的模组、整合包、资源包和光影，并提供实例管理、多种账户认证、个性化外观与 Axolotl 实验室工具。

本项目基于 [Modrinth App](https://github.com/modrinth/code) 构建，移除了不适用于本项目的商业化模块，专注于提供纯净、无广告的桌面启动体验。

本项目与客户端项目 Axolotl Client 无任何关联。

_(注：本项目是调用 Modrinth 公开 API 的独立客户端，与 Rinth, Inc. 无任何关联。)_

## 核心优势

- **真跨平台体验**：告别繁琐的环境配置，原生支持 Windows、macOS（完美兼容 Intel 与 Apple Silicon）及各类主流 Linux 发行版。
- **现代化内容生态**：集成 Modrinth 和 CurseForge，可在启动器中一键浏览。游戏实例、整合包、模组、资源包及光影均可一键安装与升级，彻底告别手动管理依赖的痛苦。
- **高度定制化**：无论是主题色调、背景图片，还是离线皮肤，核心功能与视觉展现均由你自由支配。
- **All in one 全新体验**：启动器内置 “实验室” 功能，囊括种子地图、投影工坊等海量使用工具，带来全新原生轮椅体验。

## 下载与安装

请前往 [GitHub Releases](https://github.com/Mystic-Stars/Axolotl/releases/latest) 下载适合你操作系统的最新安装包。
已安装的用户每次均可通过内置的 Tauri 签名校验机制，自动在后台完成更新，无需手动下载安装更新。

| 系统平台                | 推荐下载文件                              |
| ----------------------- | ----------------------------------------- |
| **Windows** (10/11 x64) | 下载 `.exe` (NSIS) 安装程序               |
| **macOS**               | 下载 `通用 .dmg` 镜像文件                 |
| **Linux** (x64)         | 提供 `.AppImage`，`.deb`，`.rpm` 多种格式 |

### macOS：Homebrew

可以通过社区维护的非官方 [ChiaKi-Li/cask tap](https://github.com/ChiaKi-Li/homebrew-cask) 安装：

```bash
brew install --cask ChiaKi-Li/cask/axolotl-launcher
```

如果已通过 DMG 将应用安装到 `/Applications/Axolotl Launcher.app`，可以使用 `--adopt` 让 Homebrew 接管现有应用：

```bash
brew install --cask --adopt ChiaKi-Li/cask/axolotl-launcher
```

接管前运行以下命令查看 Cask 版本，并与应用内显示的本地版本比较：

```bash
brew update
brew info --cask ChiaKi-Li/cask/axolotl-launcher
```

`--adopt` 不会升级应用。本地版本较旧时先更新应用，较新时等待 tap 更新；版本一致后再接管。若仍提示应用不一致，请核对来源和安装位置，不要用 `--force` 绕过检查。

安装或接管后，更新此应用：

```bash
brew update
brew upgrade --cask ChiaKi-Li/cask/axolotl-launcher
```

也可以使用 `brew upgrade` 更新全部由 Homebrew 管理的软件。

<details>
<summary><b>Linux 包管理器快捷安装指令</b></summary>
<br>

**Arch Linux (AUR)**：

```bash
# 源码构建版
yay -S axolotl-launcher

# 预编译二进制版
yay -S axolotl-launcher-bin
```

**Debian / Ubuntu (APT)**：

```bash
curl -fsSL https://ppa.axlmc.org/setup.sh | sudo bash
sudo apt install axolotl-launcher
```

**使用 Nix(启用Flake功能) 的任意发行版**：

> 小提示：
> 由于 Axolotl Launcher 的主分支代码更新较快，可以在下面用到的网址后面加上`/?`和参数来锁定，以免过于频繁地更新。多个参数用`&`连接。
>
> - 使用`shallow=1`避免克隆历史代码 \
>   `git+https://github.com/Mystic-Stars/Axolotl/?shallow=1`
> - 使用`ref=`指定分支（例如`experiment/linux-cef-runtime`分支）\
>   `git+https://github.com/Mystic-Stars/Axolotl/?ref=experiment/linux-cef-runtime`
> - 使用`ref=`指定版本标签（例如`v1.9.7-beta.2`）\
>   `git+https://github.com/Mystic-Stars/Axolotl/?ref=refs/tags/v1.9.7-beta.2`
> - 使用`rev=`指定提交（例如`10827ade74f49958cefaa6fda05961c7502741ef`）\
>   `git+https://github.com/Mystic-Stars/Axolotl/?rev=10827ade74f49958cefaa6fda05961c7502741ef`

```bash
nix profile add 'git+https://github.com/Mystic-Stars/Axolotl'
```

**使用 Home-Manager 的任意发行版**

将此 Flake 加入你的 `flake.nix`

```nix
# flake.nix
{
  nixConfig = {
    # 使用 Axolotl Launcher 的 Cachix 缓存。对安全性有疑问可以审查：
    # axlmc.cachix.org 以及 github.com/Axolotl-Launcher/Axolotl-Cachix
    extra-substituters = [
      "https://axlmc.cachix.org"
    ];
    extra-trusted-public-keys = [
      "axlmc.cachix.org-1:dmsRJqFQqLMfvRveerHV3DxDCW1wq1N+TX1+dryYWZU="
    ];
  };

  inputs = {
    # nixpkgs home-manager 等其他输入...
    axolotl-launcher = {
      url = "git+https://github.com/Mystic-Stars/Axolotl";
    # inputs.nixpkgs.follows = "nixpkgs";
    # ↑ 通常不建议跟随本地nixpkgs，因为nixpkgs版本不一致会难以命中缓存、从下载缓存回退成本地编译。
    };
  };
  outputs = { axolotl-launcher, <其他输入>, ... }: {
    homeConfigurations = {
      <用户名> = home-manager.lib.homeManagerConfiguration {
        modules = [
          home.nix
          axolotl-launcher.homeModules
          # 其他模块
        ];
      };
    };
  };
}
```

然后在 `home.nix` 中配置

```nix
# home.nix
{
  programs.axolotl-launcher = {
    enable = true;
  };
}
```

可选属性

| 键        | 类型    | 默认值  | 说明                                                                                                                                       |
| --------- | ------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| enable    | `bool`  | `false` | 设置为`true`以启用 Axolotl Launcher。                                                                                                      |
| launchEnv | `attrs` | `{ }`   | Axolotl Launcher 启动时额外的环境变量。例如`{ HELLO = 1; world = "two"; }`能为 Axolotl Launcher 设置额外的环境变量`HELLO=1`和`world=two`。 |

</details>

## 参与项目开发

Axolotl Launcher 的进步离不开社区的反馈与贡献。
如果遇到 Bug 或有新的功能点子，欢迎提交 Issue。如需搭建本地开发环境或查阅打包发布规范，请阅读详细的 [贡献指南 (CONTRIBUTING.md)](CONTRIBUTING.md)。
参与社区和贡献代码前，也请先阅读[行为准则 (CODE_OF_CONDUCT.md)](CODE_OF_CONDUCT.md)。

---
