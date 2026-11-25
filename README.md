# FXSearch (`FXSearch.aex`) · After Effects 原生搜索与已用效果定位插件

原生 **64-bit C/Rust AEGP (`.aex`) 插件**，无需任何外部 `.exe`、后台进程、CEP 扩展或安装脚本，直接放入 After Effects 的 `Plug-ins` 目录即可运行。

---
## 截图

![FXSearch 添加效果](image/image%2001.png)
![查找已用效果](image/image%2002.png)
![设置](image/image%2003.png)

## 安装方式

将纯净插件目录 **[FXSearch](file:///d:/Antigravity/AE搜索插件控制台/FXSearch)**（或其中的 **`FXSearch.aex`**）直接复制到：

```text
C:\Program Files\Adobe\Adobe After Effects 2027\Support Files\Plug-ins\
```
> 提示：如果你原本安装了 Video Copilot 的 `FXConsole.aex`，两者默认热键均为 `Ctrl + Space`。你可以在呼出 `FXSearch` 后点击右上角 `⚙` 将快捷键切换为 `Shift + Space` / `Alt + Space` / `` Ctrl + ` ``，或暂时移出原版 `FXConsole.aex` 避免快捷键冲突。

重启 After Effects 后即可生效（同时也会在 AE 顶部菜单 **`窗口 (Window)`** 下注册 `FXSearch` 与 `FXSearch - Locate Used Effects` 菜单项）。

---

## 核心功能与快捷键（仅在 AE 进程内生效，无全局热键）

| 快捷键 | 功能说明 |
| :--- | :--- |
| `Ctrl + Space` | 在 AE 鼠标光标位置呼出 **FXSearch**（添加效果 / `.ffx` 动画预设） |
| `Ctrl + Shift + Space` | 直达呼出 **「🎯 查找已用效果」** 模式 |
| `Tab` | 在「⚡ FXSearch · 添加」与「🎯 查找已用效果」之间切换（或在搜索框输入 `@` 直达） |
| `Alt + C` / `Ctrl + Tab` | 在「查找已用效果」模式下切换 **范围：全工程所有合成（默认） ⇄ 仅当前合成** |
| `↑` / `↓` / `PgUp` / `PgDn` | 上下移动列表选中项（支持鼠标滚轮与悬停） |
| `Enter` | 添加效果/预设到选中图层（未选中层时自动新建调整图层）；或在「查找已用效果」下**自动打开对应嵌套子合成、选中图层并展开效果属性** |
| `Alt + Enter` | 添加模式：强制新建调整图层并添加；已用效果模式：直接在列表中开关该效果 `fx` |
| `Alt + 1..9` | 快速执行当前列表第 1~9 项 |
| `Alt + F` | 在添加模式下切换筛选：全部 / 仅插件效果 / 仅 `.ffx` 预设 |
| `Ctrl + S` | 收藏 / 取消收藏当前效果或预设（`★` 置顶排序） |
| `Esc` | 关闭搜索悬浮窗并将焦点无缝归还给 AE |
