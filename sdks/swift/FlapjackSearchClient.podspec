Pod::Spec.new do |s|
  s.name = 'FlapjackSearchClient'
  s.module_name  = 'FlapjackSearchClient'
  s.version = '9.38.0'
  s.source = { :git => 'https://github.com/flapjackhq/flapjackhqsearch-client-swift.git', :tag => '9.38.0' }
  s.authors = { 'Flapjack' => 'contact@flapjackhq.com' }
  s.license = { :type => 'MIT', :file => 'LICENSE' }
  s.homepage = 'https://github.com/flapjackhq/flapjackhqsearch-client-swift/tree/main'
  s.summary = 'Flapjack Search API Client written in Swift.'
  s.documentation_url = 'https://www.flapjackhq.com/doc/libraries/sdk/install#swift'
  s.ios.deployment_target = '14.0'
  s.osx.deployment_target = '11.0'
  s.watchos.deployment_target = '7.0'
  s.tvos.deployment_target = '14.0'
  s.swift_version = '5.9'
  s.resource_bundles = { 'FlapjackSearchClient' => ['PrivacyInfo.xcprivacy']}

  s.subspec 'Core' do |subs|
    subs.source_files = 'Sources/Core/**/*.swift'
  end
  s.subspec 'Abtesting' do |subs|
    subs.source_files = 'Sources/Abtesting/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'AbtestingV3' do |subs|
    subs.source_files = 'Sources/AbtestingV3/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Analytics' do |subs|
    subs.source_files = 'Sources/Analytics/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Composition' do |subs|
    subs.source_files = 'Sources/Composition/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Ingestion' do |subs|
    subs.source_files = 'Sources/Ingestion/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Insights' do |subs|
    subs.source_files = 'Sources/Insights/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Monitoring' do |subs|
    subs.source_files = 'Sources/Monitoring/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Personalization' do |subs|
    subs.source_files = 'Sources/Personalization/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'QuerySuggestions' do |subs|
    subs.source_files = 'Sources/QuerySuggestions/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Recommend' do |subs|
    subs.source_files = 'Sources/Recommend/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
  s.subspec 'Search' do |subs|
    subs.source_files = 'Sources/Search/**/*.swift'
    subs.dependency 'FlapjackSearchClient/Core'
  end
end
