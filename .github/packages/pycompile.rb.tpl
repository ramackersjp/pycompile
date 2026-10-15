class Pycompile < Formula
  desc "Compile Python source files into standalone native binaries"
  homepage "https://github.com/ramackersjp/pycompile"
  version "@VERSION@"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ramackersjp/pycompile/releases/download/v@VERSION@/pycompile-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA_MACOS_ARM@"
    end
    on_intel do
      url "https://github.com/ramackersjp/pycompile/releases/download/v@VERSION@/pycompile-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA_MACOS_INTEL@"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ramackersjp/pycompile/releases/download/v@VERSION@/pycompile-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA_LINUX_ARM@"
    end
    on_intel do
      url "https://github.com/ramackersjp/pycompile/releases/download/v@VERSION@/pycompile-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA_LINUX_INTEL@"
    end
  end

  def install
    bin.install "pycompile"
    man1.install "pycompile.1" => "pycompile.1"
  end

  test do
    (testpath/"hello.py").write("print('Hello, World!')\n")
    system bin/"pycompile", "hello.py", "-o", "hello"
    assert_equal "Hello, World!\n", shell_output("./hello")
  end
end
