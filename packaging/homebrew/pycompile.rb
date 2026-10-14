class Pycompile < Formula
  desc "Compile Python source files into standalone native binaries"
  homepage "https://github.com/ramackersjp/pycompile"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ramackersjp/pycompile/releases/download/v0.1.0/pycompile-aarch64-apple-darwin.tar.gz"
      sha256 "0aa36fef266de367fb151d6cfd46ca991d3ecb0f60f79bd3cde302114f9221bc"
    end
    on_intel do
      url "https://github.com/ramackersjp/pycompile/releases/download/v0.1.0/pycompile-x86_64-apple-darwin.tar.gz"
      sha256 "1a384ca3b680aa566badfc88df67857bfe1a03ef63922fa3e1dd0d1529ddbd67"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ramackersjp/pycompile/releases/download/v0.1.0/pycompile-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "749abe7e67b194facf26ce9056b63a20bb9d0a3f8107c7a4b70950e852399b3b"
    end
    on_intel do
      url "https://github.com/ramackersjp/pycompile/releases/download/v0.1.0/pycompile-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "b901c1bf98bef7a4719100154b855c4b131e42c6a207d5ca0fcd4eda7ea903de"
    end
  end

  def install
    bin.install "pycompile", "python_compiler"
    man1.install "pycompile.1" => "pycompile.1"
    man1.install "pycompile.1" => "python_compiler.1"
  end

  test do
    (testpath/"hello.py").write("print('Hello, World!')\n")
    system bin/"pycompile", "hello.py", "-o", "hello"
    assert_equal "Hello, World!\n", shell_output("./hello")
  end
end
