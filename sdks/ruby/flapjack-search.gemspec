require 'English'
lib = File.expand_path('lib', __dir__)
$LOAD_PATH.unshift(lib) unless $LOAD_PATH.include?(lib)
require 'flapjack/version'
require 'date'

Gem::Specification.new do |s|
  s.name        = 'flapjack-search'
  s.version     = Flapjack::VERSION
  s.platform    = Gem::Platform::RUBY
  s.authors     = ['Flapjack Team']
  s.homepage    = 'https://github.com/flapjackhq/flapjack-search-ruby'
  s.summary     = 'Flapjack Search Ruby SDK — drop-in replacement for the Algolia Ruby client'
  s.description = 'A fully-featured Ruby API client for Flapjack Search. Drop-in replacement for the algolia gem.'
  s.licenses    = ['MIT']

  s.metadata = {
    'bug_tracker_uri' => 'https://github.com/flapjackhq/flapjack-search-ruby/issues',
    'source_code_uri' => 'https://github.com/flapjackhq/flapjack-search-ruby',
    'rubygems_mfa_required' => 'true'
  }

  s.files         = Dir['lib/**/*', 'LICENSE', 'README.md', 'MIGRATION.md', 'CHANGELOG.md'].select { |f| File.file?(f) }
  s.executables   = []
  s.require_paths = ['lib']

  s.required_ruby_version = '>= 2.6'

  s.add_dependency 'faraday', '>= 1.0.1', '< 3.0'
  s.add_dependency 'faraday-net_http_persistent', ['>= 0.15', '< 3']
  s.add_dependency 'base64', '>= 0.2.0', '< 1'

  s.add_dependency 'net-http-persistent'

  s.add_development_dependency 'bundler', '>= 2.4'
  s.add_development_dependency 'rake'
  s.add_development_dependency 'minitest', '>= 5.0'
end
