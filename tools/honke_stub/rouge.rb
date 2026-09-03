# 本家の試験を「入力の記録」のためだけに走らせる時の、rouge(色付け)の代わり
module Rouge
  def self.version = "0.0.0"
  module Lexer; def self.find(*_a) = nil; end
end
