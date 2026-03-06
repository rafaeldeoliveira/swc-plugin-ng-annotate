// Basic Angular module method annotations

// long form
angular.module("MyMod").controller("MyCtrl", function($scope, $timeout) {});

// w/ dependencies
angular.module("MyMod", ["OtherMod"]).controller("MyCtrl", function($scope, $timeout) {});

// simple
myMod.controller("foo", function($scope, $timeout) {});
myMod.service("foo", function($scope, $timeout) {});
myMod.factory("foo", function($a, $b) {});
myMod.directive("foo", function($scope, $timeout) {});
myMod.filter("foo", function($scope, $timeout) {});
myMod.animation("foo", function($scope, $timeout) {});
myMod.invoke("foo", function($scope, $timeout) {});
myMod.decorator("foo", function($scope, $timeout) {});

// no dependencies => no need to wrap
myMod.controller("foo", function() {});
myMod.factory("foo", function() {});

// run, config
myMod.run(function($scope, $timeout) {});
myMod.config(function($scope, $timeout) {});

// provider
myMod.provider("foo", function($scope) {
    this.$get = function($scope, $timeout) { bar; };
});

// directive return object
myMod.directive("foo", function($scope) {
    return {
        controller: function($scope, $timeout) { bar; }
    };
});

// component with controller object property
myMod.component("foo", {controller: function($scope, $timeout) {}});
