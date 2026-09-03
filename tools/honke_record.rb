# 本家 asciidoctor の試験を走らせて、convert に渡った入力だけを記録する。
#   ruby tools/honke_record.rb vendor/asciidoctor/test/lists_test.rb 出力.json
# 検査そのものは見ない(nokogiri の代わりを差し込むので、多くは落ちる)。
# 記録するのは 試験の名前・呼んだ helper・入力・opts の鍵。
require 'json'
$LOAD_PATH.unshift File.expand_path('honke_stub', __dir__)
$LOAD_PATH.unshift File.expand_path('../vendor/asciidoctor/lib', __dir__)
$LOAD_PATH.unshift File.expand_path('../vendor/asciidoctor/test', __dir__)
test_file, out = ARGV
# test_helper は作業ディレクトリを変えるので、径路は先に絶対にしておく
test_file = File.expand_path(test_file)
out = File.expand_path(out)
$rec = []
require 'test_helper'
module Minitest
  class Test
    %w[document_from_string block_from_string convert_string convert_string_to_embedded].each do |m|
      orig = instance_method(m)
      define_method(m) do |src, opts = {}|
        $rec << { 'test' => name, 'helper' => m, 'src' => src.to_s, 'opts' => opts.keys.map(&:to_s) }
        orig.bind(self).call(src, opts)
      end
    end
    # 検査は全部通す(見るのは入力だけ)
    %w[assert_xpath refute_xpath assert_css refute_css assert_includes refute_includes].each do |m|
      define_method(m) { |*_a| true }
    end
  end
end
Minitest.after_run { File.write(out, JSON.pretty_generate($rec)); warn "記録 #{$rec.size} 件 → #{out}" }
load test_file
