// Basic Angular module method annotations
// long form
angular.module("MyMod").controller("MyCtrl", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
// w/ dependencies
angular.module("MyMod", [
    "OtherMod"
]).controller("MyCtrl", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
// simple
myMod.controller("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.service("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.factory("foo", [
    "$a",
    "$b",
    function($a, $b) {}
]);
myMod.directive("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.filter("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.animation("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.invoke("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.decorator("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
// no dependencies => no need to wrap
myMod.controller("foo", function() {});
myMod.factory("foo", function() {});
// run, config
myMod.run([
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.config([
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
// provider
myMod.provider("foo", [
    "$scope",
    function($scope) {
        this.$get = [
            "$scope",
            "$timeout",
            function($scope, $timeout) {
                bar;
            }
        ];
    }
]);
// directive return object
myMod.directive("foo", [
    "$scope",
    function($scope) {
        return {
            controller: [
                "$scope",
                "$timeout",
                function($scope, $timeout) {
                    bar;
                }
            ]
        };
    }
]);
// component with controller object property
myMod.component("foo", {
    controller: [
        "$scope",
        "$timeout",
        function($scope, $timeout) {}
    ]
});
