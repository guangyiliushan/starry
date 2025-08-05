#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Starry编程语言构建脚本
用于自动化构建、测试和部署Starry编译器
"""

import os
import sys
import subprocess
import argparse
import shutil
from pathlib import Path

class StarryBuilder:
    def __init__(self):
        self.project_root = Path(__file__).parent.absolute()
        self.build_dir = self.project_root / "build"
        self.bin_dir = self.build_dir / "bin"
        self.lib_dir = self.build_dir / "lib"
        
    def run_command(self, cmd, cwd=None, check=True):
        """运行命令并处理输出"""
        print(f"执行命令: {' '.join(cmd)}")
        try:
            result = subprocess.run(
                cmd, 
                cwd=cwd or self.project_root,
                check=check,
                capture_output=True,
                text=True,
                encoding='utf-8'
            )
            if result.stdout:
                print(result.stdout)
            return result
        except subprocess.CalledProcessError as e:
            print(f"命令执行失败: {e}")
            if e.stderr:
                print(f"错误输出: {e.stderr}")
            sys.exit(1)
    
    def clean(self):
        """清理构建目录"""
        print("清理构建目录...")
        if self.build_dir.exists():
            shutil.rmtree(self.build_dir)
        print("清理完成")
    
    def configure(self, build_type="Release", enable_coverage=False, enable_testing=True):
        """配置CMake项目"""
        print(f"配置项目 (构建类型: {build_type})...")
        
        # 创建构建目录
        self.build_dir.mkdir(exist_ok=True)
        
        # CMake配置命令
        cmake_args = [
            "cmake",
            str(self.project_root),
            f"-DCMAKE_BUILD_TYPE={build_type}",
            f"-DENABLE_COVERAGE={'ON' if enable_coverage else 'OFF'}",
            f"-DBUILD_TESTING={'ON' if enable_testing else 'OFF'}"
        ]
        
        # 在Windows上使用Visual Studio生成器
        if sys.platform == "win32":
            cmake_args.extend(["-G", "Visual Studio 16 2019", "-A", "x64"])
        
        self.run_command(cmake_args, cwd=self.build_dir)
        print("配置完成")
    
    def build(self, target=None, parallel_jobs=None):
        """构建项目"""
        print("构建项目...")
        
        if not self.build_dir.exists():
            print("构建目录不存在，请先运行配置")
            sys.exit(1)
        
        # CMake构建命令
        build_args = ["cmake", "--build", "."]
        
        if target:
            build_args.extend(["--target", target])
        
        # 设置并行构建任务数
        if parallel_jobs:
            build_args.extend(["--parallel", str(parallel_jobs)])
        elif sys.platform != "win32":
            # 在非Windows系统上使用CPU核心数
            import multiprocessing
            build_args.extend(["--parallel", str(multiprocessing.cpu_count())])
        
        self.run_command(build_args, cwd=self.build_dir)
        print("构建完成")
    
    def test(self, test_filter=None):
        """运行测试"""
        print("运行测试...")
        
        if not self.build_dir.exists():
            print("构建目录不存在，请先构建项目")
            sys.exit(1)
        
        # CTest命令
        test_args = ["ctest", "--output-on-failure"]
        
        if test_filter:
            test_args.extend(["-R", test_filter])
        
        self.run_command(test_args, cwd=self.build_dir)
        print("测试完成")
    
    def install(self, prefix=None):
        """安装项目"""
        print("安装项目...")
        
        install_args = ["cmake", "--install", "."]
        
        if prefix:
            install_args.extend(["--prefix", prefix])
        
        self.run_command(install_args, cwd=self.build_dir)
        print("安装完成")
    
    def package(self):
        """打包项目"""
        print("打包项目...")
        
        self.run_command(["cpack"], cwd=self.build_dir)
        print("打包完成")
    
    def coverage(self):
        """生成代码覆盖率报告"""
        print("生成代码覆盖率报告...")
        
        # 检查是否启用了覆盖率
        if not (self.build_dir / "CMakeCache.txt").exists():
            print("请先使用 --enable-coverage 选项配置项目")
            sys.exit(1)
        
        # 运行测试以生成覆盖率数据
        self.test()
        
        # 生成覆盖率报告
        coverage_dir = self.build_dir / "coverage"
        coverage_dir.mkdir(exist_ok=True)
        
        if shutil.which("gcov") and shutil.which("lcov"):
            # 使用lcov生成HTML报告
            self.run_command([
                "lcov", "--capture", "--directory", ".", 
                "--output-file", "coverage.info"
            ], cwd=self.build_dir)
            
            self.run_command([
                "genhtml", "coverage.info", 
                "--output-directory", str(coverage_dir)
            ], cwd=self.build_dir)
            
            print(f"覆盖率报告已生成: {coverage_dir / 'index.html'}")
        else:
            print("未找到lcov工具，跳过HTML报告生成")
    
    def format_code(self):
        """格式化代码"""
        print("格式化代码...")
        
        if not shutil.which("clang-format"):
            print("未找到clang-format工具")
            return
        
        # 查找所有C++源文件
        cpp_files = []
        for pattern in ["**/*.cpp", "**/*.h", "**/*.hpp"]:
            cpp_files.extend(self.project_root.glob(pattern))
        
        for file_path in cpp_files:
            if "build" not in str(file_path):  # 跳过构建目录
                self.run_command(["clang-format", "-i", str(file_path)])
        
        print("代码格式化完成")
    
    def lint(self):
        """代码静态分析"""
        print("运行代码静态分析...")
        
        if shutil.which("clang-tidy"):
            # 使用clang-tidy进行静态分析
            self.run_command([
                "clang-tidy", 
                str(self.project_root / "src" / "**" / "*.cpp"),
                "--", "-std=c++17"
            ])
        else:
            print("未找到clang-tidy工具")
    
    def benchmark(self):
        """运行性能基准测试"""
        print("运行性能基准测试...")
        
        benchmark_exe = self.bin_dir / "starry_benchmark"
        if benchmark_exe.exists():
            self.run_command([str(benchmark_exe)])
        else:
            print("未找到基准测试可执行文件，请先构建项目")

def main():
    parser = argparse.ArgumentParser(description="Starry编程语言构建脚本")
    parser.add_argument("command", choices=[
        "clean", "configure", "build", "test", "install", 
        "package", "coverage", "format", "lint", "benchmark", "all"
    ], help="要执行的命令")
    
    parser.add_argument("--build-type", choices=["Debug", "Release", "RelWithDebInfo"], 
                       default="Release", help="构建类型")
    parser.add_argument("--enable-coverage", action="store_true", 
                       help="启用代码覆盖率")
    parser.add_argument("--disable-testing", action="store_true", 
                       help="禁用测试构建")
    parser.add_argument("--target", help="指定构建目标")
    parser.add_argument("--jobs", type=int, help="并行构建任务数")
    parser.add_argument("--test-filter", help="测试过滤器")
    parser.add_argument("--install-prefix", help="安装前缀")
    
    args = parser.parse_args()
    
    builder = StarryBuilder()
    
    try:
        if args.command == "clean":
            builder.clean()
        elif args.command == "configure":
            builder.configure(
                build_type=args.build_type,
                enable_coverage=args.enable_coverage,
                enable_testing=not args.disable_testing
            )
        elif args.command == "build":
            builder.build(target=args.target, parallel_jobs=args.jobs)
        elif args.command == "test":
            builder.test(test_filter=args.test_filter)
        elif args.command == "install":
            builder.install(prefix=args.install_prefix)
        elif args.command == "package":
            builder.package()
        elif args.command == "coverage":
            builder.coverage()
        elif args.command == "format":
            builder.format_code()
        elif args.command == "lint":
            builder.lint()
        elif args.command == "benchmark":
            builder.benchmark()
        elif args.command == "all":
            # 执行完整的构建流程
            builder.clean()
            builder.configure(
                build_type=args.build_type,
                enable_coverage=args.enable_coverage,
                enable_testing=not args.disable_testing
            )
            builder.build(parallel_jobs=args.jobs)
            if not args.disable_testing:
                builder.test()
            print("完整构建流程完成")
    
    except KeyboardInterrupt:
        print("\n构建被用户中断")
        sys.exit(1)
    except Exception as e:
        print(f"构建过程中发生错误: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()