// When code already has $inject arrays, adding annotations should be a no-op
Ctrl1.$inject = [
    "serviceName"
];
// @ngInject
function Ctrl1(a) {}
Ctrl1.$inject = [
    "a"
];
// @ngInject
function Ctrl2(a) {}
Ctrl2.$inject = [
    "a"
];
Ctrl2.$inject = [
    "serviceName"
];
