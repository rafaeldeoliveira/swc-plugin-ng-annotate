// Arrow function annotations
angular.module("MyMod").controller("MyCtrl", [
    "$scope",
    "$timeout",
    ($scope, $timeout)=>{}
]);
myMod.controller("foo", [
    "$scope",
    "$timeout",
    ($scope, $timeout)=>{}
]);
myMod.factory("foo", [
    "$a",
    "$b",
    ($a, $b)=>{}
]);
myMod.run([
    "$scope",
    "$timeout",
    ($scope, $timeout)=>{}
]);
myMod.config([
    "$scope",
    "$timeout",
    ($scope, $timeout)=>{}
]);
// no dependencies
myMod.controller("foo", ()=>{});
// provider with arrow $get
myMod.provider("foo", [
    "$scope",
    function($scope) {
        this.$get = [
            "$scope",
            "$timeout",
            ($scope, $timeout)=>{
                bar;
            }
        ];
    }
]);
// directive return object with arrow
myMod.directive("foo", [
    "$scope",
    ($scope)=>({
            controller: [
                "$scope",
                "$timeout",
                ($scope, $timeout)=>{
                    bar;
                }
            ]
        })
]);
