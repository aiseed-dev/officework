# 本家の試験を「入力の記録」のためだけに走らせる時の、nokogiri の代わり。
# 検査(xpath)は見ないので、何を聞かれても空を返す
module Nokogiri
  class Stub
    def method_missing(*_a) = Stub.new
    def respond_to_missing?(*_a) = true
    def to_s = ""
    def size = 0
    def each; end
  end
  def self.XML(*_a) = Stub.new
  def self.HTML(*_a) = Stub.new
  def self.HTML5(*_a) = Stub.new
  module XML; class Document; end; end
  module HTML; end
end
