{
    "$schema": "https://raw.githubusercontent.com/ScoopInstaller/Scoop/master/schema.json",
    "version": "@VERSION@",
    "description": "Compile Python source files into standalone native binaries",
    "homepage": "https://github.com/ramackersjp/pycompile",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "https://github.com/ramackersjp/pycompile/releases/download/v@VERSION@/pycompile-x86_64-pc-windows-msvc.zip",
            "hash": "@SHA_WINDOWS@",
            "extract_dir": "pycompile-x86_64-pc-windows-msvc"
        }
    },
    "bin": [
        "pycompile.exe",
        [
            "python_compiler.exe",
            "python_compiler"
        ]
    ],
    "checkver": {
        "github": "https://github.com/ramackersjp/pycompile"
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/ramackersjp/pycompile/releases/download/v$version/pycompile-x86_64-pc-windows-msvc.zip"
            }
        }
    }
}
