cmake_minimum_required(VERSION 3.16)
project(lysh C)

# ---------------------------------------------------------------------------
# lysh C 内核的构建脚本（阶段 1 + 阶段 2，完整程序）。
#
# ⚠️ 2026-xx：**验证/对拍程序已全部移除**（原 `tools/` 50 个文件）。
#    本构建脚本现在只产出三个目标：
#      lysh_core   静态内核库（唯一实现；JNI 与 CLI 都链它）
#      lysh        CLI（scan / hut / selftest）—— 筛选程序本体
#      lysh_jni    JNI 桥（→ project.NativePhase1 + project.NativePhase2）
#
#    因此 **没有 ctest 目标**。原先 23 个测试用例（逐位对拍、oracle 分界、
#    11 小屋召回、误差预算守卫等）依赖 `tools/`，已随之一并移除。
#    归档副本：`..\_archive\verify_tools_<日期>.zip`（含 50 个文件，必要时可恢复）。
#
# 怎么跑：
#   cmake -S lysh-c -B lysh-c/build/cmake
#   cmake --build lysh-c/build/cmake
#   lysh-c\build\cmake\lysh.exe scan --seed <long> [--regions N] [--threads T]
#   lysh-c\build\cmake\lysh.exe hut  --seed <long> --x X --z Z
#
# ⚠️ 改内核时的回归手段：现在只剩 CLI 自身的 `[funnel]` / `[engine]` 诊断，
#    以及 Java 侧 11 个已知小屋的端到端召回。改 `src/` 里任何东西之后，
#    至少跑一遍那 11 个坐标，确认 Y 不变。
# ---------------------------------------------------------------------------

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "" FORCE)
endif()

# ---------------------------------------------------------------------------
# 逐位一致的硬性要求：
#   必须禁止 FP contraction / 重结合。gcc/clang 用 -ffp-contract=off，
#   MSVC 用默认的 /fp:precise（绝不可用 /fp:fast）。
#   -O3 -mavx2 不影响结果（已实测），但任何 fast-math 都会破坏一致性。
# ---------------------------------------------------------------------------
if(MSVC)
  # /source-charset:utf-8 —— 必需，不是可选项。
  # 源码里的注释是 UTF-8 中文；MSVC 默认按系统 ANSI（本机 936/GBK）解码，遇到某些
  # UTF-8 序列会把**换行一起吃掉**，于是后面几行代码静默消失（实测：biome.c 里
  # `long long target[6];` 和那几个 quantize 调用整段不见，报"未声明的标识符"，
  # 而同一个文件 gcc 编译完全正常）。只改 source charset、不动 execution charset，
  # 这样 printf 里的中文诊断串仍然按系统 ANSI 输出，控制台显示不变。
  add_compile_options(/W4 /fp:precise /source-charset:utf-8)
  add_compile_definitions(_CRT_SECURE_NO_WARNINGS)
else()
  add_compile_options(-Wall -Wextra -O3 -ffp-contract=off)
  if(NOT APPLE)
    add_compile_options(-mavx2)
  endif()
  # LTO：让 lysh_dblnoise_sample_y0 能跨编译单元内联进扫描循环。
  # -ffp-contract=off 必须同时出现在【编译】和【链接】阶段 —— LTO 的代码生成发生在链接期。
  option(LYSH_LTO "Enable interprocedural optimization" ON)
  if(LYSH_LTO)
    add_compile_options(-flto)
    add_link_options(-flto -ffp-contract=off)
  endif()
endif()

# ---------------------------------------------------------------------------
# 内核库
# ---------------------------------------------------------------------------
add_library(lysh_core STATIC
  src/rng.c
  src/noise.c
  src/structure.c
  src/climate.c
  src/caves.c
  src/phase1.c
  src/biome.c
  src/spline.c
  src/terrain.c
  src/column_top.c
  src/density.c
  src/interp_noise.c
  src/aquifer.c
  src/carver.c
  src/phase2.c
  src/eval.c
  src/search.c
)
target_include_directories(lysh_core PUBLIC src)

if(NOT WIN32)
  target_link_libraries(lysh_core PUBLIC m)
endif()

# ---------------------------------------------------------------------------
# CLI（筛选程序本体：scan / hut / selftest）
# ---------------------------------------------------------------------------
add_executable(lysh src/main.c)
target_link_libraries(lysh PRIVATE lysh_core)

# ---------------------------------------------------------------------------
# JNI 壳：把阶段 1 + 阶段 2 暴露给 Java GUI
#   （project.NativePhase1 / project.NativePhase2）
# 用 JAVA_HOME 定位 jni.h；找不到就跳过。
# ---------------------------------------------------------------------------
option(LYSH_JNI "Build the JNI bridge (needs JAVA_HOME)" ON)
if(LYSH_JNI)
  if(NOT JAVA_HOME)
    set(JAVA_HOME "$ENV{JAVA_HOME}")
  endif()
  find_path(JNI_INCLUDE_DIR jni.h
    HINTS "${JAVA_HOME}/include" "C:/Java21/jdk-22/include"
    NO_DEFAULT_PATH)
  if(JNI_INCLUDE_DIR)
    add_library(lysh_jni SHARED src/jni_bridge.c)
    target_link_libraries(lysh_jni PRIVATE lysh_core)
    target_include_directories(lysh_jni PRIVATE "${JNI_INCLUDE_DIR}")
    if(WIN32)
      target_include_directories(lysh_jni PRIVATE "${JNI_INCLUDE_DIR}/win32")
    elseif(APPLE)
      target_include_directories(lysh_jni PRIVATE "${JNI_INCLUDE_DIR}/darwin")
    else()
      target_include_directories(lysh_jni PRIVATE "${JNI_INCLUDE_DIR}/linux")
    endif()
    # 输出名固定成 lysh（Windows 下即 lysh.dll），Java 侧按名字加载
    set_target_properties(lysh_jni PROPERTIES PREFIX "" OUTPUT_NAME "lysh")
    message(STATUS "lysh: JNI bridge enabled (jni.h at ${JNI_INCLUDE_DIR})")
  else()
    message(STATUS "lysh: JNI bridge skipped (set JAVA_HOME to enable)")
  endif()
endif()

message(STATUS "lysh: build type = ${CMAKE_BUILD_TYPE}")
message(STATUS "lysh: targets = lysh_core, lysh (CLI), lysh_jni ; no tests (verification tools removed)")
